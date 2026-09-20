use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::error::IpcError;
use crate::protocol::{EditorCommand, JsonRpcRequest, JsonRpcResponse};
use crate::security::verify_peer;

pub struct IpcServer {
    socket_path: PathBuf,
    running: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl IpcServer {
    /// Binds a Unix Domain Socket with strict 0600 permissions and begins listening
    /// for editor events on a background worker thread.
    pub fn bind<P: AsRef<Path>>(
        socket_path: P,
        command_tx: Sender<EditorCommand>,
    ) -> Result<Self, IpcError> {
        let path = socket_path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            // Enforce 0700 on the socket directory
            let mut dir_perms = fs::metadata(parent)?.permissions();
            dir_perms.set_mode(0o700);
            fs::set_permissions(parent, dir_perms)?;
        }

        // Clean up stale socket if present
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let listener = UnixListener::bind(&path).map_err(|e| {
            IpcError::BindError(format!("Failed to bind socket at {:?}: {}", path, e))
        })?;

        // Enforce 0600 on the socket file
        let mut file_perms = fs::metadata(&path)?.permissions();
        file_perms.set_mode(0o600);
        fs::set_permissions(&path, file_perms)?;

        // Set non-blocking mode so the worker loop can check `running` state cleanly
        listener.set_nonblocking(true)?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let path_clone = path.clone();

        let worker_handle = thread::Builder::new()
            .name("merm-ipc-worker".to_string())
            .spawn(move || {
                Self::run_listener(listener, running_clone, command_tx);
            })
            .map_err(IpcError::Io)?;

        log::info!("merm-ipc listening on {:?}", path_clone);

        Ok(Self {
            socket_path: path,
            running,
            worker_handle: Some(worker_handle),
        })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    fn run_listener(
        listener: UnixListener,
        running: Arc<AtomicBool>,
        tx: Sender<EditorCommand>,
    ) {
        while running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    // Check peer credentials with SO_PEERCRED
                    match verify_peer(&stream) {
                        Ok(creds) => {
                            log::debug!("Accepted authenticated IPC connection from PID {}", creds.pid);
                            let tx_clone = tx.clone();
                            thread::spawn(move || {
                                Self::handle_client(stream, tx_clone);
                            });
                        }
                        Err(e) => {
                            log::warn!("Rejected unauthorized connection: {}", e);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    log::error!("Error accepting IPC connection: {}", e);
                    thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }

    fn handle_client(stream: UnixStream, tx: Sender<EditorCommand>) {
        let mut writer = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to clone stream for writing: {}", e);
                return;
            }
        };

        let reader = BufReader::new(stream);

        for line_res in reader.lines() {
            match line_res {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                        Ok(req) => {
                            let command = req.parse_command();

                            // Handle ping synchronously, dispatch others to channel
                            if let EditorCommand::Ping = command {
                                let resp = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: req.id,
                                    result: Some(serde_json::Value::String("pong".to_string())),
                                    error: None,
                                };
                                if let Ok(s) = serde_json::to_string(&resp) {
                                    let _ = writeln!(writer, "{}", s);
                                    let _ = writer.flush();
                                }
                            } else {
                                if let Err(e) = tx.send(command) {
                                    log::warn!("Command receiver dropped: {}", e);
                                    break;
                                }

                                if let Some(ref req_id) = req.id {
                                    let resp = JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: Some(req_id.clone()),
                                        result: Some(serde_json::Value::Bool(true)),
                                        error: None,
                                    };
                                    if let Ok(s) = serde_json::to_string(&resp) {
                                        let _ = writeln!(writer, "{}", s);
                                        let _ = writer.flush();
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to parse JSON-RPC request '{}': {}", trimmed, e);
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Client disconnected or read error: {}", e);
                    break;
                }
            }
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}
