use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;
use std::sync::mpsc::channel;

use merm_core::ThemeId;
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

fn config_path() -> PathBuf {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home).join("merm").join("config.toml")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home).join(".config").join("merm").join("config.toml")
    } else {
        PathBuf::from(".config/merm/config.toml")
    }
}

fn load_theme_from_config() -> Option<ThemeId> {
    let path = config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("theme") {
                    if let Some(val) = trimmed.split('=').nth(1) {
                        let name = val.trim().trim_matches('"').trim_matches('\'');
                        return ThemeId::from_name(name);
                    }
                }
            }
        }
    } else {
        // Create default config file template
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
            let default_config = r#"# merm configuration file
# Default theme: catppuccin-mocha, tokyo-night, nord, gruvbox, dracula, latte
theme = "catppuccin-mocha"

# Default diagram layout direction (TD, LR, RL, BT)
direction = "TD"
"#;
            let _ = fs::write(&path, default_config);
        }
    }
    None
}

fn print_usage() {
    eprintln!(
        r#"merm: High-Performance Native Linux Diagram Viewer for Modal Editors

USAGE:
    merm [OPTIONS] [FILE]

ARGS:
    <FILE>    Path to Markdown or Mermaid file (reads from stdin if '-' or omitted)

OPTIONS:
    -t, --theme <NAME>   Set color theme (catppuccin, tokyo-night, nord, gruvbox, dracula, latte)
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
    t                    Cycle color themes (Mocha -> Tokyo Night -> Nord -> Gruvbox -> Dracula -> Latte)
    n, N                 Select next / previous node
    / or f               Fuzzy search nodes
    Mouse Drag           Pan canvas
    Mouse Wheel          Zoom in / out
    q, Esc               Quit

CONFIG:
    ~/.config/merm/config.toml
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
    let mut cli_theme: Option<ThemeId> = None;

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
            "-t" | "--theme" => {
                if i + 1 < args.len() {
                    cli_theme = ThemeId::from_name(&args[i + 1]);
                    if cli_theme.is_none() {
                        eprintln!("Unknown theme '{}'. Options: catppuccin, tokyo-night, nord, gruvbox, dracula, latte", args[i + 1]);
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --theme requires a theme name");
                    process::exit(1);
                }
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

    // Determine active theme (CLI override > Config file > Default Catppuccin Mocha)
    let active_theme = cli_theme.or_else(load_theme_from_config).unwrap_or(ThemeId::CatppuccinMocha);

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

    // Initialize AppState with active theme
    let mut app = AppState::with_theme(content, force_software_render, active_theme);
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
