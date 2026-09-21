use std::io::Write;
use std::process::{Command, Stdio};

/// Copies text to the system clipboard across Wayland, X11, and other platforms.
/// Tries `arboard` first, and if unavailable or failing, tries `wl-copy`, `xclip`, or `xsel`.
pub fn copy_to_clipboard(text: &str) -> bool {
    // 1. Try native arboard clipboard
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        if clipboard.set_text(text.to_string()).is_ok() {
            log::info!(
                "Successfully copied {} bytes to clipboard via arboard",
                text.len()
            );
            return true;
        }
    }

    // 2. Linux Wayland fallback: wl-copy
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if let Ok(mut child) = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    log::info!("Successfully copied {} bytes via wl-copy", text.len());
                    return true;
                }
            }
        }
    }

    // 3. Linux X11 fallbacks: xclip / xsel
    for (bin, args) in &[
        ("xclip", &["-selection", "clipboard"][..]),
        ("xsel", &["--clipboard", "--input"][..]),
    ] {
        if let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if let Ok(status) = child.wait() {
                if status.success() {
                    log::info!("Successfully copied {} bytes via {}", text.len(), bin);
                    return true;
                }
            }
        }
    }

    log::warn!("Could not copy text to clipboard: all providers failed");
    false
}
