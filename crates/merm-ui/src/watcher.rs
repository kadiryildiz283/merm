use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum WatcherEvent {
    SourceChanged(Vec<PathBuf>),
}

pub struct ProjectWatcher {
    _watcher: RecommendedWatcher,
}

impl ProjectWatcher {
    pub fn new<P: AsRef<Path>>(
        watch_path: P,
        event_tx: Sender<WatcherEvent>,
    ) -> Result<Self, String> {
        let (notify_tx, notify_rx) = channel();

        let mut watcher = RecommendedWatcher::new(notify_tx, Config::default())
            .map_err(|e| format!("Failed to create notify watcher: {e}"))?;

        watcher
            .watch(watch_path.as_ref(), RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch path {:?}: {e}", watch_path.as_ref()))?;

        // Dedicated debounce thread: 250ms window
        thread_builder_spawn(move || {
            let debounce_dur = Duration::from_millis(250);
            let mut last_change = Instant::now();
            let mut pending_paths: Vec<PathBuf> = Vec::new();
            let mut has_pending = false;

            loop {
                let timeout = if has_pending {
                    let elapsed = last_change.elapsed();
                    if elapsed >= debounce_dur {
                        let paths = std::mem::take(&mut pending_paths);
                        let _ = event_tx.send(WatcherEvent::SourceChanged(paths));
                        has_pending = false;
                        Duration::from_millis(500)
                    } else {
                        debounce_dur - elapsed
                    }
                } else {
                    Duration::from_millis(500)
                };

                match notify_rx.recv_timeout(timeout) {
                    Ok(Ok(event)) => {
                        let is_mutation = matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                        );
                        if is_mutation {
                            for path in event.paths {
                                if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                                    let path_str = path.to_string_lossy();
                                    if !path_str.contains("/target/")
                                        && !path_str.contains("/.git/")
                                        && !path_str.contains("/.merm/")
                                    {
                                        pending_paths.push(path);
                                        last_change = Instant::now();
                                        has_pending = true;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        log::warn!("Filesystem watcher event error: {:?}", e);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if has_pending && last_change.elapsed() >= debounce_dur {
                            let paths = std::mem::take(&mut pending_paths);
                            let _ = event_tx.send(WatcherEvent::SourceChanged(paths));
                            has_pending = false;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        Ok(Self { _watcher: watcher })
    }
}

fn thread_builder_spawn<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let _ = std::thread::Builder::new()
        .name("merm-fs-watcher".to_string())
        .spawn(f);
}
