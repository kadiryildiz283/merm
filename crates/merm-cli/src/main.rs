use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;
use std::sync::mpsc::channel;

use merm_ipc::IpcServer;
use merm_ui::{AppState, MermAppWindow};

fn default_socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let pid = process::id();

    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("merm").join(format!("merm-{}.sock", pid))
    } else {
        PathBuf::from(format!("/tmp/merm-{}", uid)).join(format!("merm-{}.sock", pid))
    }
}

fn print_usage() {
    eprintln!(
        r#"merm: High-Performance Native Linux Diagram Viewer for Modal Editors

USAGE:
    merm [OPTIONS] [FILE]

ARGS:
    <FILE>    Path to Markdown or Mermaid file (reads from stdin if '-' or omitted)

OPTIONS:
    --software-render    Force CPU software rasterization fallback (softbuffer/tiny-skia)
    --headless           Run in headless mode without opening a GUI window
    --socket <PATH>      Custom Unix Domain Socket path for editor IPC
    -h, --help           Print help information
    -V, --version        Print version information

VIM KEYBINDINGS (Modal Navigation in GUI):
    h, j, k, l           Pan left, down, up, right
    +, -                 Zoom in, zoom out
    0                    Reset view (fit to screen)
    p, Tab               Pivot diagram direction (TD -> LR -> RL -> BT)
    n, N                 Select next / previous node
    / or f               Fuzzy search nodes
    Mouse Drag           Pan canvas
    Mouse Wheel          Zoom in / out
    q, Esc               Quit
"#
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let mut file_path: Option<String> = None;
    let mut socket_path = default_socket_path();
    let mut force_software_render = false;
    let mut headless = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            "-V" | "--version" => {
                println!("merm v0.1.0 (Linux x86_64/aarch64 native)");
                return Ok(());
            }
            "--software-render" => {
                force_software_render = true;
            }
            "--headless" => {
                headless = true;
            }
            "--socket" => {
                if i + 1 < args.len() {
                    socket_path = PathBuf::from(&args[i + 1]);
                    i += 1;
                } else {
                    eprintln!("Error: --socket requires a path argument");
                    process::exit(1);
                }
            }
            arg if !arg.starts_with('-') => {
                file_path = Some(arg.to_string());
            }
            unknown => {
                eprintln!("Unknown option: {}", unknown);
                print_usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    // Read diagram content
    let content = match file_path.as_deref() {
        Some("-") | None => {
            if atty_is_terminal() {
                // Interactive fallback sample if run without input in terminal
                r#"classDiagram
    direction LR
    class BankAccount {
        +String owner
        -BigDecimal balance
        +deposit(amount) bool
        +withdraw(amount) bool
    }
    class CheckingAccount {
        +BigDecimal overdraftLimit
        +processCheck()
    }
    BankAccount <|-- CheckingAccount : inherits
    BankAccount *-- Transaction : contains
"#
                .to_string()
            } else {
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            }
        }
        Some(path) => fs::read_to_string(path)?,
    };

    // Initialize IPC Server with SO_PEERCRED protection
    let (ipc_tx, ipc_rx) = channel();
    let _ipc_server = match IpcServer::bind(&socket_path, ipc_tx) {
        Ok(server) => {
            log::info!("IPC server bound to {:?}", socket_path);
            Some(server)
        }
        Err(e) => {
            log::warn!("Could not bind IPC server ({}). Continuing without editor sync.", e);
            None
        }
    };

    // Initialize AppState
    let mut app = AppState::new(content, force_software_render);
    log::info!("Status: {}", app.hud_status());

    let has_display = env::var("WAYLAND_DISPLAY").is_ok() || env::var("DISPLAY").is_ok();

    if headless || !has_display {
        println!("\n[merm] Running in headless mode. Socket: {:?}", socket_path);
        println!("[merm] Active HUD: {}", app.hud_status());

        // Drain single tick
        while let Ok(cmd) = ipc_rx.try_recv() {
            app.handle_ipc_command(cmd);
        }

        let frame = app.render_current_frame();
        if let Some(res) = frame {
            log::info!(
                "Render frame passed: Backend={:?}, Objects={}",
                res.backend,
                res.rendered_objects
            );
        }
    } else {
        println!("\n[merm] Launching interactive GUI window. Socket: {:?}", socket_path);
        println!("[merm] Active HUD: {}", app.hud_status());

        let window = MermAppWindow::new(app, Some(ipc_rx));
        window.run()?;
    }

    Ok(())
}

fn atty_is_terminal() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}
