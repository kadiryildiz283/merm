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
            std::time::Duration::from_millis(16),
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
                let avail_h = if self.app_state.show_split_buffer {
                    (height as f32 * 0.50).max(100.0)
                } else {
                    (height.saturating_sub(hud_height)).max(1) as f32
                };
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
        let w = width as f32;
        let h = height as f32;
        let mut svg = String::with_capacity(8192);
        svg.push_str(&format!(
            r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">"#,
            width, height, width, height
        ));

        // Dynamic HiDPI scaling based on viewport size (e.g. 2.8K / 4K monitors)
        let ui_scale = if w >= 2500.0 || h >= 1500.0 {
            1.85f32
        } else if w >= 1800.0 || h >= 1000.0 {
            1.35f32
        } else {
            1.0f32
        };

        // 1. Top Header Bar (Scalable: 26px * ui_scale)
        let top_h = 26.0 * ui_scale;
        let top_font_size = (11.5 * ui_scale).round() as u32;
        let top_text_y = top_h * 0.65;
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{}" height="{}" fill="{}" opacity="0.96"/>"##,
            w, top_h, palette.card_bg
        ));
        svg.push_str(&format!(
            r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            top_h, w, top_h, palette.badge_bg
        ));

        // Project badge & bound classes
        let project_badge = if let Some(ref m) = app_state.manifest {
            format!(
                "⚡ MERM │ 📁 {} ({} classes)",
                m.project_name,
                m.bindings.len()
            )
        } else {
            "⚡ MERM │ [No project bound - run &set]".to_string()
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">{}</text>"##,
            12.0 * ui_scale, top_text_y, palette.method_color, top_font_size, escape_xml(&project_badge)
        ));

        // Active node indicator in top bar
        if let Some(ref sel_id) = app_state.active_node_id {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">🎯 &lt;{}&gt; [t: Test │ i: Inspect │ e: Edit]</text>"##,
                380.0 * ui_scale, top_text_y, palette.stereotype_color, top_font_size, escape_xml(sel_id)
            ));
        }

        // Right side indicators (Direction, Theme, Help hint)
        let dir_str = app_state.active_direction.as_str();
        let theme_str = app_state.theme.palette().name;
        let right_header = format!(
            "Dir: [{}] ('p') │ Theme: [{}] ('T') │ :help",
            dir_str, theme_str
        );
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">{}</text>"##,
            w - 12.0 * ui_scale, top_text_y, palette.text_sub, top_font_size, escape_xml(&right_header)
        ));

        // 2. Full-screen / Drawer Modals: NodeTest & Inspector
        match app_state.modal.mode {
            UiMode::NodeTest => {
                let drawer_h = 145.0 * ui_scale;
                let drawer_y = h - drawer_h;
                let active_node = app_state
                    .modal
                    .active_test_node_id
                    .as_deref()
                    .unwrap_or("Unknown");

                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.97"/>"##,
                    drawer_y, w, drawer_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
                    drawer_y, w, drawer_y, palette.method_color
                ));

                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">🚀 Executable Node Test Harness: &lt;{}&gt;</text>"##,
                    16.0 * ui_scale, drawer_y + 26.0 * ui_scale, palette.method_color, (13.0 * ui_scale).round() as u32, escape_xml(active_node)
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[Enter] Run Harness │ [Esc] Close</text>"##,
                    w - 16.0 * ui_scale, drawer_y + 26.0 * ui_scale, palette.text_sub, (11.0 * ui_scale).round() as u32
                ));

                // Input box
                let input_disp = format!(
                    "Payload Input: {}█",
                    escape_xml(&app_state.modal.test_input_buffer)
                );
                let input_h = 32.0 * ui_scale;
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" stroke="{}" stroke-width="1"/>"##,
                    14.0 * ui_scale, drawer_y + 40.0 * ui_scale, w - 28.0 * ui_scale, input_h, palette.background, palette.method_color
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                    24.0 * ui_scale, drawer_y + 61.0 * ui_scale, palette.text_main, (12.5 * ui_scale).round() as u32, input_disp
                ));

                // Status or output preview
                let status_preview = if let Some(ref res) = app_state.last_execution_result {
                    let st = if res.success { "✔ PASS" } else { "✖ FAIL" };
                    format!(
                        "Execution Result: [{}] ({}ms) -> {}",
                        st,
                        res.duration_ms,
                        escape_xml(&res.output_payload)
                    )
                } else {
                    "Type JSON or string argument for node entrypoint. Press [Enter] to run."
                        .to_string()
                };
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                    16.0 * ui_scale, drawer_y + 98.0 * ui_scale, palette.text_sub, (11.5 * ui_scale).round() as u32, status_preview
                ));

                svg.push_str("</svg>");
                return Some(svg);
            }
            UiMode::Inspector => {
                let modal_w = (w - 160.0 * ui_scale).max(520.0 * ui_scale);
                let modal_h = (h - 120.0 * ui_scale).max(360.0 * ui_scale);
                let modal_x = (w - modal_w) / 2.0;
                let modal_y = (h - modal_h) / 2.0;

                svg.push_str(&format!(
                    r##"<rect x="0" y="0" width="{}" height="{}" fill="#000000" opacity="0.65"/>"##,
                    w, h
                ));

                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="2"/>"##,
                    modal_x, modal_y, modal_w, modal_h, palette.background, palette.badge_bg
                ));

                let header_bar_h = 36.0 * ui_scale;
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}"/>"##,
                    modal_x, modal_y, modal_w, header_bar_h, palette.badge_bg
                ));

                let active_id = app_state.active_node_id.as_deref().unwrap_or("Unknown");
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">🔍 Node Inspector: &lt;{}&gt;</text>"##,
                    modal_x + 16.0 * ui_scale, modal_y + 24.0 * ui_scale, palette.text_main, (13.0 * ui_scale).round() as u32, escape_xml(active_id)
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[e] Edit in $EDITOR │ [t] Test │ [Esc/q] Close</text>"##,
                    modal_x + modal_w - 16.0 * ui_scale, modal_y + 24.0 * ui_scale, palette.text_sub, (11.0 * ui_scale).round() as u32
                ));

                let node_data = app_state
                    .current_diagram
                    .as_ref()
                    .and_then(|d| d.nodes.iter().find(|n| n.id == active_id));

                let binding = app_state
                    .manifest
                    .as_ref()
                    .and_then(|m| m.get_binding(active_id));

                let mut cur_y = modal_y + 60.0 * ui_scale;
                let role = node_data
                    .and_then(|n| n.stereotype.as_deref())
                    .unwrap_or("struct");
                let bound_file = binding.map(|b| b.file.as_str()).unwrap_or("src/lib.rs");
                let entrypoint = binding
                    .and_then(|b| b.entrypoint.as_deref())
                    .unwrap_or("run");
                let is_exec = binding.map(|b| b.executable).unwrap_or(true);

                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">Kind: &lt;&lt;{}&gt;&gt; │ File: {} │ Executable: {} │ Entrypoint: {}()</text>"##,
                    modal_x + 16.0 * ui_scale, cur_y, palette.stereotype_color, (12.0 * ui_scale).round() as u32, escape_xml(role), escape_xml(bound_file), if is_exec { "YES" } else { "NO" }, escape_xml(entrypoint)
                ));
                cur_y += 24.0 * ui_scale;

                if let Some(doc) = node_data.and_then(|n| n.doc_comment.as_deref()) {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-style="italic">%% {}</text>"##,
                        modal_x + 16.0 * ui_scale, cur_y, palette.comment_color, (11.0 * ui_scale).round() as u32, escape_xml(doc)
                    ));
                    cur_y += 20.0 * ui_scale;
                }

                if let Some(node) = node_data {
                    if !node.attributes.is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">Fields ({}):</text>"##,
                            modal_x + 16.0 * ui_scale, cur_y, palette.var_color, (11.5 * ui_scale).round() as u32, node.attributes.len()
                        ));
                        cur_y += 18.0 * ui_scale;
                        for attr in node.attributes.iter().take(4) {
                            let type_str = attr.type_name.as_deref().unwrap_or("String");
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">   {}{}: {}</text>"##,
                                modal_x + 20.0 * ui_scale, cur_y, palette.text_main, (11.0 * ui_scale).round() as u32, attr.visibility, escape_xml(&attr.name), escape_xml(type_str)
                            ));
                            cur_y += 17.0 * ui_scale;
                        }
                    }

                    if !node.methods.is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">Methods ({}):</text>"##,
                            modal_x + 16.0 * ui_scale, cur_y, palette.method_color, (11.5 * ui_scale).round() as u32, node.methods.len()
                        ));
                        cur_y += 18.0 * ui_scale;
                        for meth in node.methods.iter().take(4) {
                            let ret_str = meth
                                .type_name
                                .as_ref()
                                .map(|r| format!(" -> {}", r))
                                .unwrap_or_default();
                            svg.push_str(&format!(
                                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">   {}{}{}</text>"##,
                                modal_x + 20.0 * ui_scale, cur_y, palette.text_main, (11.0 * ui_scale).round() as u32, meth.visibility, escape_xml(&meth.name), escape_xml(&ret_str)
                            ));
                            cur_y += 17.0 * ui_scale;
                        }
                    }
                }

                svg.push_str("</svg>");
                return Some(svg);
            }
            _ => {}
        }

        let status_h = 24.0 * ui_scale;
        let cmd_h = 28.0 * ui_scale;
        let bottom_bars_h = status_h + cmd_h;

        // 3. Persistent Vim Horizontal Split Buffer (when show_split_buffer is true or mode is Report)
        let is_split_open = app_state.show_split_buffer || app_state.modal.mode == UiMode::Report;
        if is_split_open {
            let split_h = (h * 0.48).clamp(240.0 * ui_scale, (h - bottom_bars_h - 40.0).max(120.0));
            let split_y = h - split_h;

            // Split Buffer Background
            svg.push_str(&format!(
                r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>"##,
                split_y, w, split_h - bottom_bars_h, palette.card_bg
            ));

            // Split Header Line (Top Border)
            let split_header_h = 26.0 * ui_scale;
            let split_header_font = (12.0 * ui_scale).round() as u32;
            let split_header_y = split_y + split_header_h * 0.65;
            svg.push_str(&format!(
                r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>"##,
                split_y, w, split_header_h, palette.badge_bg
            ));
            svg.push_str(&format!(
                r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
                split_y, w, split_y, palette.method_color
            ));

            let title = "🤖 AI Architecture & Diagnostic Buffer (Vim Split)";
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">── [ {} ] ──</text>"##,
                14.0 * ui_scale, split_header_y, palette.method_color, split_header_font, escape_xml(title)
            ));
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[i/&amp;/: Prompt │ j/k, Wheel: Scroll │ &amp;ok: Apply │ s: Toggle Split │ :q: Quit]</text>"##,
                w - 14.0 * ui_scale, split_header_y, palette.text_sub, (11.0 * ui_scale).round() as u32
            ));

            // Vertical gutter separator line
            let gutter_w = 48.0 * ui_scale;
            let gutter_x = gutter_w;
            let content_top_y = split_y + split_header_h + 8.0 * ui_scale;
            let content_bottom_y = h - bottom_bars_h - 4.0 * ui_scale;

            svg.push_str(&format!(
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                gutter_x,
                split_y + split_header_h,
                gutter_x,
                h - bottom_bars_h,
                palette.badge_bg
            ));

            // Render lines with line numbers and syntax highlighting
            if let Some(ref content) = app_state.report_content {
                let all_lines: Vec<&str> = content.lines().collect();
                let total_lines = all_lines.len();
                let line_h = 18.0 * ui_scale;
                let max_visible_lines =
                    ((content_bottom_y - content_top_y) / line_h).floor() as usize;

                let max_offset = total_lines.saturating_sub(max_visible_lines);
                let offset = app_state.modal.report_scroll_offset.min(max_offset);

                let line_font = (12.5 * ui_scale).round() as u32;
                let gutter_font = (11.0 * ui_scale).round() as u32;

                let mut cur_y = content_top_y + 12.0 * ui_scale;
                for (idx, line) in all_lines
                    .iter()
                    .skip(offset)
                    .take(max_visible_lines)
                    .enumerate()
                {
                    let line_no = offset + idx + 1;

                    // Gutter line number
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">{}</text>"##,
                        gutter_x - 8.0 * ui_scale, cur_y, palette.text_sub, gutter_font, line_no
                    ));

                    // Syntax coloring
                    let color = if line.starts_with("#") || line.starts_with("===") {
                        &palette.stereotype_color
                    } else if line.starts_with("✔")
                        || line.starts_with("Status: PASS")
                        || line.starts_with("SUCCESS")
                        || line.starts_with("+ ")
                    {
                        &palette.method_color
                    } else if line.starts_with("✖")
                        || line.starts_with("Status: FAIL")
                        || line.starts_with("FAILED")
                        || line.starts_with("- ")
                    {
                        &palette.var_color
                    } else if line.starts_with("```") {
                        &palette.method_color
                    } else if line.starts_with("---") {
                        &palette.badge_bg
                    } else if line.starts_with("* ") {
                        &palette.stereotype_color
                    } else {
                        &palette.text_main
                    };

                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                        gutter_x + 12.0 * ui_scale, cur_y, color, line_font, escape_xml(line)
                    ));

                    cur_y += line_h;
                }

                // Scroll Indicator in header
                let scroll_status = if total_lines <= max_visible_lines {
                    "All".to_string()
                } else if offset == 0 {
                    "Top".to_string()
                } else if offset >= max_offset {
                    "Bot".to_string()
                } else {
                    format!("{:.0}%", (offset as f32 / total_lines as f32) * 100.0)
                };
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[{}]</text>"##,
                    w - 460.0 * ui_scale, split_header_y, palette.stereotype_color, (11.0 * ui_scale).round() as u32, scroll_status
                ));
            }
        }

        // 4. Authentic Vim Statusline (Height: status_h at y = h - bottom_bars_h)
        let status_y = h - bottom_bars_h;
        let status_font = (11.5 * ui_scale).round() as u32;
        let status_text_y = status_y + status_h * 0.65;
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>"##,
            status_y, w, status_h, palette.badge_bg
        ));

        // Mode Badge
        let (badge_bg, badge_text) = match app_state.modal.mode {
            UiMode::Command => ("#fab387", "COMMAND"),
            UiMode::NodeTest => ("#cba6f7", "TEST"),
            UiMode::Search => ("#f9e2af", "SEARCH"),
            UiMode::Report => ("#a6e3a1", "REPORT"),
            _ => ("#89b4fa", "NORMAL"),
        };
        let badge_w: f32 = 84.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>"##,
            status_y, badge_w, status_h, badge_bg
        ));
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="#11111b" font-family="monospace" font-size="{}" font-weight="bold" text-anchor="middle">{}</text>"##,
            badge_w / 2.0, status_text_y, status_font, badge_text
        ));

        // Project & Focus File
        let project_name = app_state
            .manifest
            .as_ref()
            .map(|m| m.project_name.as_str())
            .unwrap_or("unbound");
        let active_sym = app_state.active_node_id.as_deref().unwrap_or("canvas");
        let file_info = format!(" 📁 {}  {}", project_name, active_sym);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">{}</text>"##,
            badge_w + 10.0 * ui_scale, status_text_y, palette.text_main, status_font, escape_xml(&file_info)
        ));

        // Middle: Busy animation or status message
        let middle_x: f32 = (badge_w + 240.0 * ui_scale).min(w - 280.0 * ui_scale);
        if app_state.is_busy {
            let busy_text = format!("⏳ {} [working...]", app_state.busy_message);
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">{}</text>"##,
                middle_x, status_text_y, palette.method_color, status_font, escape_xml(&busy_text)
            ));
        } else if !app_state.status_message.is_empty() {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                middle_x, status_text_y, palette.text_sub, status_font, escape_xml(&app_state.status_message)
            ));
        }

        // Right: Metrics ruler
        let zoom_pct = (app_state.transform.scale * 100.0) as u32;
        let ruler = format!("utf-8 │ 120 FPS │ {}% │ Ln 1, Col 1", zoom_pct);
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">{}</text>"##,
            w - 12.0 * ui_scale, status_text_y, palette.text_sub, status_font, escape_xml(&ruler)
        ));

        // 5. Vim Command Line (Height: cmd_h at y = h - cmd_h)
        let cmd_y = h - cmd_h;
        let cmd_font = (13.0 * ui_scale).round() as u32;
        let cmd_text_y = cmd_y + cmd_h * 0.65;
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>"##,
            cmd_y, w, cmd_h, palette.card_bg
        ));
        svg.push_str(&format!(
            r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            cmd_y, w, cmd_y, palette.badge_bg
        ));

        if app_state.modal.mode == UiMode::Command {
            let cmd_str = format!("{}█", escape_xml(&app_state.modal.command_buffer));
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">{}</text>"##,
                12.0 * ui_scale, cmd_text_y, palette.text_main, cmd_font, cmd_str
            ));
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[Enter] Run  │  [Esc] Cancel</text>"##,
                w - 14.0 * ui_scale, cmd_text_y, palette.text_sub, (11.0 * ui_scale).round() as u32
            ));
        } else {
            // Normal / Report mode prompt hint
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">[AI Chat Buffer] Type 'i', '&amp;', or ':' to prompt AI / run commands (&amp;check, &amp;ai, &amp;advice, &amp;ok, &amp;set) │ 's': toggle split █</text>"##,
                12.0 * ui_scale, cmd_text_y, palette.text_sub, (12.0 * ui_scale).round() as u32
            ));
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">merm (Vim mode)</text>"##,
                w - 14.0 * ui_scale, cmd_text_y, palette.badge_bg, (11.0 * ui_scale).round() as u32
            ));
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
                .with_inner_size(winit::dpi::LogicalSize::new(1600.0, 1000.0))
                .with_maximized(true)
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
                    Key::Named(NamedKey::Space) => {
                        self.app_state.modal.handle_key(' ', has_selected)
                    }
                    Key::Named(NamedKey::Backspace) => self.app_state.modal.handle_backspace(),
                    Key::Named(NamedKey::Delete) => self.app_state.modal.handle_backspace(),
                    Key::Named(NamedKey::Enter) => {
                        self.app_state.modal.handle_key('\n', has_selected)
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_add(1);
                            UiAction::None
                        } else {
                            UiAction::Pan { dx: 0.0, dy: -30.0 }
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_sub(1);
                            UiAction::None
                        } else {
                            UiAction::Pan { dx: 0.0, dy: 30.0 }
                        }
                    }
                    Key::Named(NamedKey::PageDown) => {
                        if self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_add(10);
                            UiAction::None
                        } else {
                            UiAction::Zoom { factor: 0.85 }
                        }
                    }
                    Key::Named(NamedKey::PageUp) => {
                        if self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_sub(10);
                            UiAction::None
                        } else {
                            UiAction::Zoom { factor: 1.15 }
                        }
                    }
                    Key::Named(NamedKey::Escape) => {
                        if self.app_state.modal.mode != UiMode::Normal {
                            self.app_state.modal.handle_key('\x1b', has_selected)
                        } else {
                            if self.app_state.active_node_id.is_some() {
                                self.app_state.active_node_id = None;
                            }
                            UiAction::None
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
                        if !self.app_state.is_running {
                            event_loop.exit();
                            return;
                        }
                        if let Some(ref w) = self.window {
                            let title = format!("merm | {}", self.app_state.hud_status());
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (win_w, win_h) = self.current_surface_size;
                let (cx, cy) = self.last_cursor_pos.unwrap_or((640.0, 360.0));

                let ui_scale = if win_w >= 2500 || win_h >= 1500 {
                    1.85f64
                } else if win_w >= 1800 || win_h >= 1000 {
                    1.35f64
                } else {
                    1.0f64
                };
                let bottom_bars_h = (24.0 + 28.0) * ui_scale;
                let split_top_y =
                    (win_h as f64 * 0.48).clamp(240.0 * ui_scale, (win_h as f64 - bottom_bars_h - 40.0).max(120.0));
                let is_split_open =
                    self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report;
                if is_split_open && cy >= (win_h as f64 - split_top_y)
                {
                    let scroll_delta = match delta {
                        MouseScrollDelta::LineDelta(_, y) => {
                            if y > 0.0 {
                                -3
                            } else {
                                3
                            }
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            if pos.y > 0.0 {
                                -2
                            } else {
                                2
                            }
                        }
                    };
                    if scroll_delta < 0 {
                        self.app_state.modal.report_scroll_offset = self
                            .app_state
                            .modal
                            .report_scroll_offset
                            .saturating_sub((-scroll_delta) as usize);
                    } else {
                        self.app_state.modal.report_scroll_offset = self
                            .app_state
                            .modal
                            .report_scroll_offset
                            .saturating_add(scroll_delta as usize);
                    }
                    if let Some(ref w) = self.window {
                        w.request_redraw();
                    }
                    return;
                }

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
                    let ui_scale = if win_w >= 2500 || win_h >= 1500 {
                        1.85f64
                    } else if win_w >= 1800 || win_h >= 1000 {
                        1.35f64
                    } else {
                        1.0f64
                    };
                    let cmd_h = 28.0 * ui_scale;

                    // Click on bottom command line
                    if win_h > 0 && cy >= (win_h as f64 - cmd_h) {
                        if self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.mode = UiMode::Command;
                            if self.app_state.modal.command_buffer.is_empty() {
                                self.app_state.modal.command_buffer = "&".to_string();
                            }
                        } else {
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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

        if self.app_state.is_busy {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(16),
            ));
            needs_redraw = true;
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(50),
            ));
        }

        if !self.app_state.is_running {
            event_loop.exit();
            return;
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
