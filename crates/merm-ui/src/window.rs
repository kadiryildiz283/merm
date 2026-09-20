use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use merm_core::escape_xml;
use merm_ipc::EditorCommand;
use merm_render::SvgRasterizer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::app_state::AppState;
use crate::modal::{UiAction, UiMode};
use crate::watcher::{ProjectWatcher, WatcherEvent};
use crate::worker::{AsyncWorker, WorkerResult};

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

pub struct MermAppWindow {
    app_state: AppState,
    ipc_rx: Option<Receiver<EditorCommand>>,
    worker_rx: Option<Receiver<WorkerResult>>,
    watcher_rx: Option<Receiver<WatcherEvent>>,
    _watcher: Option<ProjectWatcher>,
    rasterizer: SvgRasterizer,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    last_frame: Instant,
    mouse_dragging: bool,
    last_cursor_pos: Option<(f64, f64)>,
    initial_fit_done: bool,
    dragging_node_idx: Option<usize>,
    drag_node_offset: (f32, f32),
    current_surface_size: (u32, u32),
}

impl MermAppWindow {
    pub fn new(mut app_state: AppState, ipc_rx: Option<Receiver<EditorCommand>>) -> Self {
        let (worker_res_tx, worker_res_rx) = std::sync::mpsc::channel();
        let worker = AsyncWorker::spawn(worker_res_tx);
        app_state.worker = Some(worker);

        let (watcher_tx, watcher_rx) = std::sync::mpsc::channel();
        let watcher = if let Some(ref m) = app_state.manifest {
            ProjectWatcher::new(&m.project_root, watcher_tx).ok()
        } else {
            None
        };

        Self {
            app_state,
            ipc_rx,
            worker_rx: Some(worker_res_rx),
            watcher_rx: Some(watcher_rx),
            _watcher: watcher,
            rasterizer: SvgRasterizer::new(),
            window: None,
            context: None,
            surface: None,
            last_frame: Instant::now(),
            mouse_dragging: false,
            last_cursor_pos: None,
            initial_fit_done: false,
            dragging_node_idx: None,
            drag_node_offset: (0.0, 0.0),
            current_surface_size: (0, 0),
        }
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::wait_duration(
            std::time::Duration::from_millis(50),
        ));
        let mut app = self;
        event_loop.run_app(&mut app)?;
        Ok(())
    }

    fn redraw(&mut self) {
        let window = match self.window.as_ref() {
            Some(w) => w.clone(),
            None => return,
        };

        let surface = match self.surface.as_mut() {
            Some(s) => s,
            None => return,
        };

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        if self.current_surface_size != (width, height) {
            surface
                .resize(
                    std::num::NonZeroU32::new(width).unwrap(),
                    std::num::NonZeroU32::new(height).unwrap(),
                )
                .expect("Failed to resize softbuffer surface");
            self.current_surface_size = (width, height);
        }

        let mut buffer = surface
            .buffer_mut()
            .expect("Failed to get softbuffer buffer");

        // Drain any incoming IPC commands from Neovim / Helix
        if let Some(ref rx) = self.ipc_rx {
            while let Ok(cmd) = rx.try_recv() {
                self.app_state.handle_ipc_command(cmd);
            }
        }

        let bg_color = parse_hex_color(&self.app_state.theme.palette().background);
        let hud_height = 32u32;

        if !self.initial_fit_done {
            if let Some(ref diagram) = self.app_state.current_diagram {
                let avail_h = (height.saturating_sub(hud_height)).max(1) as f32;
                self.app_state.transform.fit_to_viewport(
                    diagram.width,
                    diagram.height,
                    width as f32,
                    avail_h,
                );
                self.initial_fit_done = true;
            }
        }

        // Always fill background first
        buffer.fill(bg_color);

        // Rasterize active diagram SVG using tiny-skia + resvg
        if let Some(ref diagram) = self.app_state.current_diagram {
            if let Err(e) = self.rasterizer.rasterize(
                &diagram.svg,
                &self.app_state.transform,
                width,
                height,
                &mut buffer,
                Some(bg_color),
            ) {
                log::error!("Rasterization failed: {}", e);
            }
        }

        // Render vector UI Overlay (HUD bar, Command input, NodeTest drawer, Report modal)
        if let Some(overlay_svg) = Self::build_overlay_svg(&self.app_state, width, height) {
            if let Err(e) =
                self.rasterizer
                    .rasterize_overlay(&overlay_svg, width, height, &mut buffer)
            {
                log::warn!("Overlay rasterization warning: {}", e);
            }
        }

        buffer.present().expect("Failed to present buffer");
        self.last_frame = Instant::now();
    }

    pub fn build_overlay_svg(app_state: &AppState, width: u32, height: u32) -> Option<String> {
        let palette = app_state.theme.palette();
        let mut svg = String::new();
        svg.push_str(&format!(
            r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">"#,
            width, height, width, height
        ));

        // 1. Top Header Bar
        let top_h = 32.0;
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{}" height="{}" fill="{}" opacity="0.95"/>"##,
            width, top_h, palette.card_bg
        ));
        svg.push_str(&format!(
            r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            top_h, width, top_h, palette.badge_bg
        ));

        // Brand + Project info
        let project_badge = if let Some(ref m) = app_state.manifest {
            format!(
                "⚡ MERM | Project: {} ({} bound classes)",
                m.project_name,
                m.bindings.len()
            )
        } else {
            "⚡ MERM | [No project bound - run &set to connect]".to_string()
        };
        svg.push_str(&format!(
            r##"<text x="14" y="21" fill="{}" font-family="monospace" font-size="12" font-weight="bold">{}</text>"##,
            palette.method_color, escape_xml(&project_badge)
        ));

        // Active node indicator in top bar
        if let Some(ref sel_id) = app_state.active_node_id {
            svg.push_str(&format!(
                r##"<text x="420" y="21" fill="{}" font-family="monospace" font-size="12">🎯 Active: &lt;{}&gt; [t: Test | i: Inspect | e: Edit]</text>"##,
                palette.stereotype_color, escape_xml(sel_id)
            ));
        }

        // Right side indicators (Direction, Theme, Mode)
        let dir_str = app_state.active_direction.as_str();
        let theme_str = app_state.theme.palette().name;
        let mode_badge = match app_state.modal.mode {
            UiMode::Normal => "NORMAL",
            UiMode::Command => "COMMAND",
            UiMode::NodeTest => "TEST",
            UiMode::Inspector => "INSPECT",
            UiMode::Report => "REPORT",
            UiMode::Search => "SEARCH",
            _ => "VIEW",
        };
        let right_header = format!(
            "Dir: [{}] ('p') | Theme: [{}] ('T') | [{}]",
            dir_str, theme_str, mode_badge
        );
        svg.push_str(&format!(
            r##"<text x="{}" y="21" fill="{}" font-family="monospace" font-size="11" text-anchor="end">{}</text>"##,
            width as f32 - 14.0, palette.text_sub, escape_xml(&right_header)
        ));

        // 2. Bottom Command Dock / Modals
        match app_state.modal.mode {
            UiMode::Command => {
                let dock_h = 46.0;
                let dock_y = height as f32 - dock_h;
                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>"##,
                    dock_y, width, dock_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
                    dock_y, width, dock_y, palette.method_color
                ));

                let input_w = (width as f32 - 250.0).max(200.0);
                let input_box_h = 32.0;
                let input_y = dock_y + 7.0;
                svg.push_str(&format!(
                    r##"<rect x="12" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1.8"/>"##,
                    input_y, input_w, input_box_h, palette.background, palette.method_color
                ));

                let cmd_text = format!("{}█", escape_xml(&app_state.modal.command_buffer));
                svg.push_str(&format!(
                    r##"<text x="24" y="{}" fill="{}" font-family="monospace" font-size="14" font-weight="bold">{}</text>"##,
                    input_y + 21.0, palette.text_main, cmd_text
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" text-anchor="end">[Enter] Run  |  [Esc] Cancel</text>"##,
                    width as f32 - 16.0, dock_y + 28.0, palette.text_sub
                ));
            }
            UiMode::NodeTest => {
                let drawer_h = 110.0;
                let drawer_y = height as f32 - drawer_h;
                let active_node = app_state
                    .modal
                    .active_test_node_id
                    .as_deref()
                    .unwrap_or("Unknown");

                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.95"/>"##,
                    drawer_y, width, drawer_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
                    drawer_y, width, drawer_y, palette.method_color
                ));

                // Title line
                svg.push_str(&format!(
                    r##"<text x="16" y="{}" fill="{}" font-family="monospace" font-size="14" font-weight="bold">🚀 Test Node Harness: &lt;{}&gt;</text>"##,
                    drawer_y + 26.0, palette.method_color, escape_xml(active_node)
                ));

                // Input line
                let input_disp =
                    format!("Input: {}█", escape_xml(&app_state.modal.test_input_buffer));
                svg.push_str(&format!(
                    r##"<text x="16" y="{}" fill="{}" font-family="monospace" font-size="13">{}</text>"##,
                    drawer_y + 54.0, palette.text_main, input_disp
                ));

                // Status or output preview
                let status_preview = if let Some(ref res) = app_state.last_execution_result {
                    let st = if res.success { "✔ PASS" } else { "✖ FAIL" };
                    format!(
                        "Result: [{}] ({}ms) -> {}",
                        st,
                        res.duration_ms,
                        escape_xml(&res.output_payload)
                    )
                } else {
                    "Type JSON or string input. Press [Enter] to run test harness, [Esc] to exit."
                        .to_string()
                };

                svg.push_str(&format!(
                    r##"<text x="16" y="{}" fill="{}" font-family="monospace" font-size="12">{}</text>"##,
                    drawer_y + 82.0, palette.text_sub, status_preview
                ));
            }
            UiMode::Report => {
                // Floating modal centered on screen
                let modal_w = (width as f32 - 120.0).max(400.0);
                let modal_h = (height as f32 - 120.0).max(300.0);
                let modal_x = (width as f32 - modal_w) / 2.0;
                let modal_y = (height as f32 - modal_h) / 2.0;

                // Dim backdrop
                svg.push_str(&format!(
                    r##"<rect x="0" y="0" width="{}" height="{}" fill="#000000" opacity="0.6"/>"##,
                    width, height
                ));

                // Modal dialog window
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="2"/>"##,
                    modal_x, modal_y, modal_w, modal_h, palette.background, palette.badge_bg
                ));

                // Header bar
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="36" rx="8" fill="{}"/>"##,
                    modal_x, modal_y, modal_w, palette.badge_bg
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="14" font-weight="bold">Diagnostic &amp; Architecture Report</text>"##,
                    modal_x + 16.0, modal_y + 23.0, palette.text_main
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" text-anchor="end">Press [Esc] or [q] to close</text>"##,
                    modal_x + modal_w - 16.0, modal_y + 23.0, palette.text_sub
                ));

                // Report text lines
                if let Some(ref content) = app_state.report_content {
                    let mut line_y = modal_y + 60.0;
                    for line in content.lines().take(28) {
                        let color = if line.starts_with("✔") || line.starts_with("Status: PASS") {
                            &palette.method_color
                        } else if line.starts_with("✖") || line.starts_with("Status: FAIL") {
                            &palette.var_color
                        } else if line.starts_with("===") || line.starts_with("---") {
                            &palette.badge_bg
                        } else {
                            &palette.text_main
                        };

                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12">{}</text>"##,
                            modal_x + 20.0, line_y, color, escape_xml(line)
                        ));
                        line_y += 18.0;
                    }
                }
            }
            UiMode::Inspector => {
                // Centered Node Inspector modal window
                let modal_w = (width as f32 - 160.0).max(480.0);
                let modal_h = (height as f32 - 120.0).max(340.0);
                let modal_x = (width as f32 - modal_w) / 2.0;
                let modal_y = (height as f32 - modal_h) / 2.0;

                // Dim backdrop
                svg.push_str(&format!(
                    r##"<rect x="0" y="0" width="{}" height="{}" fill="#000000" opacity="0.65"/>"##,
                    width, height
                ));

                // Modal dialog window
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="2"/>"##,
                    modal_x, modal_y, modal_w, modal_h, palette.background, palette.badge_bg
                ));

                // Header bar
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="36" rx="8" fill="{}"/>"##,
                    modal_x, modal_y, modal_w, palette.badge_bg
                ));

                let active_id = app_state.active_node_id.as_deref().unwrap_or("Unknown");
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="14" font-weight="bold">🔍 Node Inspector: &lt;{}&gt;</text>"##,
                    modal_x + 16.0, modal_y + 23.0, palette.text_main, escape_xml(active_id)
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" text-anchor="end">[e] Edit | [t] Test | [Esc/q] Close</text>"##,
                    modal_x + modal_w - 16.0, modal_y + 23.0, palette.text_sub
                ));

                let node_data = app_state
                    .current_diagram
                    .as_ref()
                    .and_then(|d| d.nodes.iter().find(|n| n.id == active_id));

                let binding = app_state
                    .manifest
                    .as_ref()
                    .and_then(|m| m.get_binding(active_id));

                let mut cur_y = modal_y + 60.0;

                let role = node_data
                    .and_then(|n| n.stereotype.as_deref())
                    .unwrap_or("struct");
                let bound_file = binding
                    .map(|b| b.file.as_str())
                    .unwrap_or_else(|| "src/lib.rs (default)");
                let entrypoint = binding
                    .and_then(|b| b.entrypoint.as_deref())
                    .unwrap_or("run");
                let is_exec = binding.map(|b| b.executable).unwrap_or(true);

                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">Type: &lt;&lt;{}&gt;&gt; | File: {} | Executable: {} | Entrypoint: {}</text>"##,
                    modal_x + 20.0, cur_y, palette.stereotype_color, escape_xml(role), escape_xml(bound_file), if is_exec { "YES" } else { "NO" }, escape_xml(entrypoint)
                ));
                cur_y += 24.0;

                if let Some(doc) = node_data.and_then(|n| n.doc_comment.as_deref()) {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" font-style="italic">%% {}</text>"##,
                        modal_x + 20.0, cur_y, palette.comment_color, escape_xml(doc)
                    ));
                    cur_y += 20.0;
                }

                if let Some(node) = node_data {
                    if !node.attributes.is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">Fields / State ({}):</text>"##,
                            modal_x + 20.0, cur_y, palette.var_color, node.attributes.len()
                        ));
                        cur_y += 18.0;
                        for attr in node.attributes.iter().take(5) {
                            let type_str = attr.type_name.as_deref().unwrap_or("String");
                            let comm_str = attr
                                .comment
                                .as_ref()
                                .map(|c| format!(" // {}", c))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11">   {}{}: {}{}</text>"##,
                                modal_x + 24.0, cur_y, palette.text_main, attr.visibility, escape_xml(&attr.name), escape_xml(type_str), escape_xml(&comm_str)
                            ));
                            cur_y += 16.0;
                        }
                    }

                    if !node.methods.is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">Methods / Functions ({}):</text>"##,
                            modal_x + 20.0, cur_y, palette.method_color, node.methods.len()
                        ));
                        cur_y += 18.0;
                        for meth in node.methods.iter().take(5) {
                            let ret_str = meth
                                .type_name
                                .as_ref()
                                .map(|r| format!(" -> {}", r))
                                .unwrap_or_default();
                            let comm_str = meth
                                .comment
                                .as_ref()
                                .map(|c| format!(" // {}", c))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11">   {}{}{}{}</text>"##,
                                modal_x + 24.0, cur_y, palette.text_main, meth.visibility, escape_xml(&meth.name), escape_xml(&ret_str), escape_xml(&comm_str)
                            ));
                            cur_y += 16.0;
                        }
                    }
                }

                if let Some(ref diag) = app_state.current_diagram {
                    let outgoing: Vec<_> =
                        diag.edges.iter().filter(|e| e.from == active_id).collect();
                    let incoming: Vec<_> =
                        diag.edges.iter().filter(|e| e.to == active_id).collect();
                    if !outgoing.is_empty() || !incoming.is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">Architectural Relations:</text>"##,
                            modal_x + 20.0, cur_y, palette.stereotype_color
                        ));
                        cur_y += 18.0;
                        for e in outgoing.iter().take(3) {
                            let lbl = e
                                .label
                                .as_ref()
                                .map(|l| format!(" : {}", l))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11">   --&gt; {}{}</text>"##,
                                modal_x + 24.0, cur_y, palette.text_sub, escape_xml(&e.to), escape_xml(&lbl)
                            ));
                            cur_y += 16.0;
                        }
                        for e in incoming.iter().take(3) {
                            let lbl = e
                                .label
                                .as_ref()
                                .map(|l| format!(" : {}", l))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11">   &lt;-- {}{}</text>"##,
                                modal_x + 24.0, cur_y, palette.text_sub, escape_xml(&e.from), escape_xml(&lbl)
                            ));
                            cur_y += 16.0;
                        }
                    }
                }

                cur_y += 6.0;
                let exec_info = if let Some(ref res) = app_state.last_execution_result {
                    let st = if res.success { "✔ PASS" } else { "✖ FAIL" };
                    format!(
                        "Last Harness Execution: [{}] ({}ms) Output: {}",
                        st, res.duration_ms, res.output_payload
                    )
                } else {
                    "Last Harness Execution: [Not executed yet. Press 't' to run]".to_string()
                };
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11" font-weight="bold">{}</text>"##,
                    modal_x + 20.0, cur_y, palette.method_color, escape_xml(&exec_info)
                ));
            }
            _ => {
                // Normal mode Command Dock (Height: 46px)
                let dock_h = 46.0;
                let dock_y = height as f32 - dock_h;
                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.95"/>"##,
                    dock_y, width, dock_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>"##,
                    dock_y, width, dock_y, palette.badge_bg
                ));

                // Command Input Box with prompt and cursor
                let input_w = (width as f32 - 465.0).max(180.0);
                let input_box_h = 30.0;
                let input_y = dock_y + 8.0;
                svg.push_str(&format!(
                    r##"<rect x="12" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1.2"/>"##,
                    input_y, input_w, input_box_h, palette.background, palette.badge_bg
                ));

                let prompt_placeholder =
                    ": / & Commands (&check, &ai, &advice, &agy, &set, :cfg) or click... █";
                svg.push_str(&format!(
                    r##"<text x="24" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">{}</text>"##,
                    input_y + 19.0, palette.text_sub, escape_xml(prompt_placeholder)
                ));

                // Quick Action Pills
                let pills_start_x = (width as f32 - 445.0).max(10.0);
                let pills: [(&str, f32, &str); 8] = [
                    ("&check", 54.0, &palette.method_color),
                    ("&ai", 38.0, &palette.stereotype_color),
                    ("&advice", 62.0, &palette.text_main),
                    ("&agy", 48.0, &palette.type_color),
                    ("&set", 46.0, &palette.var_color),
                    (":test", 50.0, &palette.method_color),
                    (":cfg", 44.0, &palette.text_sub),
                    (":help", 48.0, &palette.text_sub),
                ];

                let mut cur_px = pills_start_x;
                for (label, p_w, color) in pills {
                    if cur_px + p_w > width as f32 - 8.0 {
                        break;
                    }
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="28" rx="5" fill="{}" stroke="{}" stroke-width="1"/>"##,
                        cur_px, input_y + 1.0, p_w, palette.badge_bg, color
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11" font-weight="bold" text-anchor="middle">{}</text>"##,
                        cur_px + p_w / 2.0, input_y + 18.0, color, escape_xml(label)
                    ));
                    cur_px += p_w + 6.0;
                }
            }
        }

        svg.push_str("</svg>");
        Some(svg)
    }
}

impl ApplicationHandler for MermAppWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let initial_title = format!("merm | {}", self.app_state.hud_status());
            let win_attr = Window::default_attributes()
                .with_title(initial_title)
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
                .with_transparent(true);

            let window = Arc::new(
                event_loop
                    .create_window(win_attr)
                    .expect("Failed to create winit window"),
            );

            let context = softbuffer::Context::new(window.clone())
                .expect("Failed to create softbuffer context");
            let surface = softbuffer::Surface::new(&context, window.clone())
                .expect("Failed to create softbuffer surface");

            window.request_redraw();

            self.window = Some(window);
            self.context = Some(context);
            self.surface = Some(surface);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::Resized(_) => {
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                let has_selected = self.app_state.active_node_id.is_some();
                let action = match logical_key {
                    Key::Character(c) => {
                        let ch = c.chars().next().unwrap_or(' ');
                        self.app_state.modal.handle_key(ch, has_selected)
                    }
                    Key::Named(NamedKey::Backspace) => self.app_state.modal.handle_backspace(),
                    Key::Named(NamedKey::Enter) => {
                        self.app_state.modal.handle_key('\n', has_selected)
                    }
                    Key::Named(NamedKey::Escape) => {
                        if self.app_state.modal.mode != UiMode::Normal {
                            self.app_state.modal.handle_key('\x1b', has_selected)
                        } else {
                            UiAction::Quit
                        }
                    }
                    Key::Named(NamedKey::Tab) => {
                        if self.app_state.modal.mode == UiMode::Normal {
                            UiAction::PivotDirection
                        } else {
                            UiAction::None
                        }
                    }
                    _ => UiAction::None,
                };

                match action {
                    UiAction::Quit => {
                        event_loop.exit();
                    }
                    UiAction::None => {
                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    }
                    other => {
                        self.app_state.handle_key_action(other);
                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let factor = match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        if y > 0.0 {
                            1.15
                        } else {
                            0.85
                        }
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        if pos.y > 0.0 {
                            1.10
                        } else {
                            0.90
                        }
                    }
                };

                let (cx, cy) = self.last_cursor_pos.unwrap_or((640.0, 360.0));
                self.app_state
                    .transform
                    .zoom_at(factor, cx as f32, cy as f32);
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(idx) = self.dragging_node_idx {
                    let (world_x, world_y) = self
                        .app_state
                        .transform
                        .screen_to_world(position.x as f32, position.y as f32);
                    let new_x = world_x - self.drag_node_offset.0;
                    let new_y = world_y - self.drag_node_offset.1;
                    self.app_state.move_node(idx, new_x, new_y);
                    if let Some(ref w) = self.window {
                        w.request_redraw();
                    }
                } else if self.mouse_dragging {
                    if let Some((last_x, last_y)) = self.last_cursor_pos {
                        let dx = (position.x - last_x) as f32;
                        let dy = (position.y - last_y) as f32;
                        self.app_state.transform.pan(dx, dy);
                        if let Some(ref w) = self.window {
                            w.request_redraw();
                        }
                    }
                }
                self.last_cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state == ElementState::Pressed {
                    let (cx, cy) = self.last_cursor_pos.unwrap_or((640.0, 360.0));
                    let (win_w, win_h) = self.current_surface_size;

                    // 1. Check click on Bottom Command Dock
                    if win_h > 0 && cy >= (win_h as f64 - 46.0) {
                        let pills_start_x = (win_w as f64 - 445.0).max(10.0);
                        if cx >= pills_start_x {
                            let rel_x = cx - pills_start_x;
                            if rel_x < 54.0 {
                                // &check
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = "&check".to_string();
                                self.app_state.handle_key_action(UiAction::ExecuteCommand(
                                    "&check".to_string(),
                                ));
                            } else if rel_x < 60.0 + 38.0 {
                                // &ai
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = "&ai ".to_string();
                            } else if rel_x < 104.0 + 62.0 {
                                // &advice
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = "&advice ".to_string();
                            } else if rel_x < 172.0 + 48.0 {
                                // &agy
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = "&agy ".to_string();
                            } else if rel_x < 226.0 + 46.0 {
                                // &set
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = "&set ".to_string();
                            } else if rel_x < 278.0 + 50.0 {
                                // :test
                                if self.app_state.active_node_id.is_some() {
                                    self.app_state.modal.mode = UiMode::NodeTest;
                                    self.app_state
                                        .handle_key_action(UiAction::SetMode(UiMode::NodeTest));
                                } else {
                                    self.app_state.modal.mode = UiMode::Command;
                                    self.app_state.modal.command_buffer = ":test ".to_string();
                                }
                            } else if rel_x < 334.0 + 44.0 {
                                // :cfg
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = ":config".to_string();
                                self.app_state.handle_key_action(UiAction::ExecuteCommand(
                                    ":config".to_string(),
                                ));
                            } else {
                                // :help
                                self.app_state.modal.mode = UiMode::Command;
                                self.app_state.modal.command_buffer = ":help".to_string();
                                self.app_state.handle_key_action(UiAction::ExecuteCommand(
                                    ":help".to_string(),
                                ));
                            }
                        } else {
                            // Clicked command input box
                            self.app_state.modal.mode = UiMode::Command;
                            if self.app_state.modal.command_buffer.is_empty() {
                                self.app_state.modal.command_buffer = ":".to_string();
                            }
                        }

                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                        return;
                    }

                    let (world_x, world_y) = self
                        .app_state
                        .transform
                        .screen_to_world(cx as f32, cy as f32);

                    let hit_idx = if let Some(ref diag) = self.app_state.current_diagram {
                        diag.nodes.iter().position(|n| {
                            world_x >= n.x
                                && world_x <= n.x + n.width
                                && world_y >= n.y
                                && world_y <= n.y + n.height
                        })
                    } else {
                        None
                    };

                    if let Some(idx) = hit_idx {
                        let (node_id, node_label, nx, ny, attr_count, meth_count, comment_preview) = {
                            let n = &self.app_state.current_diagram.as_ref().unwrap().nodes[idx];
                            let comm = n
                                .attributes
                                .iter()
                                .chain(n.methods.iter())
                                .find_map(|m| m.comment.clone());
                            (
                                n.id.clone(),
                                n.label.clone(),
                                n.x,
                                n.y,
                                n.attributes.len(),
                                n.methods.len(),
                                comm,
                            )
                        };
                        self.dragging_node_idx = Some(idx);
                        self.drag_node_offset = (world_x - nx, world_y - ny);
                        self.mouse_dragging = false;

                        self.app_state.select_node(Some(&node_id));
                        if attr_count > 0 || meth_count > 0 {
                            if let Some(comm) = comment_preview {
                                self.app_state.status_message = format!(
                                    "Class: {} ({} vars, {} methods) | // {} | Press 't' to test",
                                    node_id, attr_count, meth_count, comm
                                );
                            } else {
                                self.app_state.status_message = format!(
                                    "Class: {} ({} vars, {} methods) | Drag to move | Press 't' to test",
                                    node_id, attr_count, meth_count
                                );
                            }
                        } else {
                            self.app_state.status_message = format!(
                                "Selected: [{}] (drag to move | Press 't' to test)",
                                node_label
                            );
                        }
                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    } else {
                        self.dragging_node_idx = None;
                        self.mouse_dragging = true;
                        self.app_state.select_node(None);
                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    }
                } else {
                    self.dragging_node_idx = None;
                    self.mouse_dragging = false;
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let mut needs_redraw = false;

        if let Some(ref rx) = self.ipc_rx {
            while let Ok(cmd) = rx.try_recv() {
                self.app_state.handle_ipc_command(cmd);
                needs_redraw = true;
            }
        }

        if let Some(ref rx) = self.worker_rx {
            while let Ok(res) = rx.try_recv() {
                self.app_state.handle_worker_result(res);
                needs_redraw = true;
            }
        }

        if let Some(ref rx) = self.watcher_rx {
            while let Ok(evt) = rx.try_recv() {
                match evt {
                    WatcherEvent::SourceChanged(paths) => {
                        self.app_state.handle_source_files_changed(&paths);
                        needs_redraw = true;
                    }
                }
            }
        }

        if needs_redraw {
            if let Some(ref w) = self.window {
                let title = format!("merm | {}", self.app_state.hud_status());
                w.set_title(&title);
                w.request_redraw();
            }
        }
    }
}
