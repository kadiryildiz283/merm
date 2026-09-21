use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;
use std::sync::mpsc::channel;

use merm_core::{ProjectManifest, RustScanner, ThemeId};
use merm_ipc::IpcServer;
use merm_ui::{AppState, MermAppWindow};

fn default_socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    let pid = process::id();

    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir)
            .join("merm")
            .join(format!("merm-{}.sock", pid))
    } else {
        PathBuf::from(format!("/tmp/merm-{}", uid)).join(format!("merm-{}.sock", pid))
    }
}

fn config_path() -> PathBuf {
    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(config_home).join("merm").join("config.toml")
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("merm")
            .join("config.toml")
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
# Default theme: monokai, terminal, catppuccin-mocha, tokyo-night, nord, gruvbox, dracula, latte
theme = "monokai"

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
        r#"merm: High-Performance Native Linux Architecture & Diagram Studio

USAGE:
    merm [OPTIONS] [FILE_OR_PROJECT_DIR]

ARGS:
    <FILE_OR_PROJECT_DIR>  Path to Markdown/Mermaid file, or Rust project root directory to bind

OPTIONS:
    -t, --theme <NAME>   Set color theme (catppuccin, tokyo-night, nord, gruvbox, dracula, monokai, terminal, latte)
    --software-render    Force CPU software rasterization fallback (softbuffer/tiny-skia)
    --headless           Run in headless mode without opening a GUI window
    --socket <PATH>      Custom Unix Domain Socket path for editor IPC
    -h, --help           Print help information
    -V, --version        Print version information

INTERACTIVE COMMAND PROTOCOL (& / : prefix):
    &set [PATH]          Bind current Mermaid diagram to a Rust project root (.merm/manifest.json)
    &check               Verify compatibility between Rust project and Mermaid (AST + build + LLM)
    &advice <PROMPT>     Request architectural advice and proposal from LLM (read-only)
    &ok                  Apply recommendation from &advice with automatic snapshot and rollback
    &ai <PROMPT>         Autonomous multi-file refactoring/scaffolding with cargo check verification
    :add <kind> <Name>   Add class, struct, or enum to diagram and scaffold Rust file
    :connect <A> <B>     Connect two nodes with dependency arrow
    :test [Node] [Input] Execute node test harness with input/output capture
    :help                Show interactive command guide

KEYBINDINGS (NORMAL mode):
    : or &               Open interactive command bar
    t                    Execute test harness on selected node (input/output drawer)
    T                    Cycle color themes
    a or o               Add new class / struct
    c                    Connect nodes
    e, E                 Open interactive Node Editor drawer
    g                    Open bound Rust source file in $EDITOR
    h, j, k, l           Pan left, down, up, right
    +, -                 Zoom in, zoom out
    0                    Reset view (fit to screen)
    Arrows               Navigate diagram nodes in 2D direction (Right/Left/Down/Up)
    Tab, n, N            Select next / previous node
    p                    Pivot diagram direction (TD -> LR -> RL -> BT)
    / or f               Fuzzy search nodes
    Mouse Drag           Move node or pan canvas
    Mouse Wheel          Zoom in / out
    q, Esc               Quit / close modal
"#
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("usvg", log::LevelFilter::Error)
        .filter_module("resvg", log::LevelFilter::Error)
        .init();

    let args: Vec<String> = env::args().collect();
    let mut file_path: Option<String> = None;
    let mut output_path: Option<PathBuf> = None;
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
                        eprintln!("Unknown theme '{}'. Options: catppuccin, tokyo-night, nord, gruvbox, dracula, monokai, terminal, latte", args[i + 1]);
                    }
                    i += 1;
                } else {
                    eprintln!("Error: --theme requires a theme name");
                    process::exit(1);
                }
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                } else {
                    eprintln!("Error: --output requires a file path");
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
            "-" => {
                file_path = Some("-".to_string());
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

    // Determine active theme (CLI override > Config file > Default Monokai)
    let active_theme = cli_theme
        .or_else(load_theme_from_config)
        .unwrap_or(ThemeId::Monokai);

    // Read diagram content
    let mut bound_dir: Option<String> = None;
    let content = match file_path.as_deref() {
        Some("-") | None => {
            if atty_is_terminal() {
                // Check current directory or any ancestor for project root
                let curr_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                if let Some(project_root) = ProjectManifest::detect_project_root(&curr_dir) {
                    bound_dir = Some(project_root.to_string_lossy().to_string());
                    if let Ok(scan_report) = RustScanner::scan_project(&project_root) {
                        if !scan_report.symbols.is_empty() {
                            RustScanner::generate_mermaid_class_diagram(&scan_report)
                        } else {
                            format!(
                                "classDiagram\n    direction TD\n    class {} {{\n        +run()\n    }}\n",
                                project_root
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("App")
                            )
                        }
                    } else {
                        "classDiagram\n    direction TD\n    class App {\n        +run()\n    }\n"
                            .to_string()
                    }
                } else {
                    "classDiagram\n    direction TD\n    class NewNode {\n        +run()\n    }\n"
                        .to_string()
                }
            } else {
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            }
        }
        Some(path) => {
            let p = std::path::Path::new(path);
            if p.is_dir() {
                if let Some(project_root) = ProjectManifest::detect_project_root(p) {
                    bound_dir = Some(project_root.to_string_lossy().to_string());
                    if let Ok(scan_report) = RustScanner::scan_project(&project_root) {
                        if !scan_report.symbols.is_empty() {
                            RustScanner::generate_mermaid_class_diagram(&scan_report)
                        } else {
                            format!(
                                "classDiagram\n    direction TD\n    class {} {{\n        +run()\n    }}\n",
                                project_root
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("App")
                            )
                        }
                    } else {
                        "classDiagram\n    direction TD\n    class App {\n        +run()\n    }\n"
                            .to_string()
                    }
                } else {
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("App");
                    format!(
                        "classDiagram\n    direction TD\n    class {} {{\n        +run()\n    }}\n",
                        name
                    )
                }
            } else {
                if let Some(project_root) = ProjectManifest::detect_project_root(p) {
                    bound_dir = Some(project_root.to_string_lossy().to_string());
                }
                let raw_content = fs::read_to_string(path)?;
                if raw_content.trim().is_empty() {
                    let default_name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("NewNode");
                    let clean_name = if default_name == "mermaid" {
                        "App"
                    } else {
                        default_name
                    };
                    format!(
                        "classDiagram\n    direction TD\n    class {} {{\n        +run()\n    }}\n",
                        clean_name
                    )
                } else {
                    raw_content
                }
            }
        }
    };

    // Initialize IPC Server with SO_PEERCRED protection
    let (ipc_tx, ipc_rx) = channel();
    let _ipc_server = match IpcServer::bind(&socket_path, ipc_tx) {
        Ok(server) => {
            log::info!("IPC server bound to {:?}", socket_path);
            Some(server)
        }
        Err(e) => {
            log::warn!(
                "Could not bind IPC server ({}). Continuing without editor sync.",
                e
            );
            None
        }
    };

    // Initialize AppState with active theme
    let mut app = AppState::with_theme(content, force_software_render, active_theme);
    if let Some(dir) = bound_dir {
        let _ = app.bind_project(Some(&dir));
    }
    log::info!("Status: {}", app.hud_status());

    let has_display = env::var("WAYLAND_DISPLAY").is_ok() || env::var("DISPLAY").is_ok();

    if headless || !has_display {
        println!(
            "\n[merm] Running in headless mode. Socket: {:?}",
            socket_path
        );
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

        if let Some(ref out_path) = output_path {
            if let Some(ref diag) = app.current_diagram {
                let ext = out_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("png");
                if ext == "svg" {
                    fs::write(out_path, &diag.svg)?;
                    println!("[merm] Exported diagram SVG to {:?}", out_path);
                } else {
                    let rasterizer = merm_render::SvgRasterizer::new();
                    let bg_color = parse_hex_color(&app.theme.palette().background);
                    match rasterizer.rasterize_to_png(&diag.svg, 2.0, Some(bg_color)) {
                        Ok(png_bytes) => {
                            fs::write(out_path, png_bytes)?;
                            println!("[merm] Rendered and exported diagram to {:?}", out_path);
                        }
                        Err(e) => {
                            eprintln!("Failed to rasterize PNG: {}", e);
                        }
                    }
                }
            }
        }
    } else {
        println!(
            "\n[merm] Launching interactive GUI window. Socket: {:?}",
            socket_path
        );
        println!("[merm] Active HUD: {}", app.hud_status());

        let window = MermAppWindow::new(app, Some(ipc_rx));
        window.run()?;
    }

    Ok(())
}

fn parse_hex_color(hex: &str) -> u32 {
    let s = hex.trim().to_lowercase();
    if s == "transparent" || s == "none" {
        return 0x00000000;
    }
    let clean = s.trim_start_matches('#');
    match clean.len() {
        6 => {
            if let Ok(rgb) = u32::from_str_radix(clean, 16) {
                0xff00_0000 | rgb
            } else {
                0xff1e_1e2e
            }
        }
        8 => u32::from_str_radix(clean, 16).unwrap_or(0xff1e_1e2e),
        _ => 0xff1e_1e2e,
    }
}

fn atty_is_terminal() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}
