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
    pub fn new(app_state: AppState, ipc_rx: Option<Receiver<EditorCommand>>) -> Self {
        Self {
            app_state,
            ipc_rx,
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

    fn build_overlay_svg(app_state: &AppState, width: u32, height: u32) -> Option<String> {
        let palette = app_state.theme.palette();
        let mut svg = String::new();
        svg.push_str(&format!(
            r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">"#,
            width, height, width, height
        ));

        // 1. Bottom HUD / Command bar
        match app_state.modal.mode {
            UiMode::Command => {
                let bar_h = 36.0;
                let bar_y = height as f32 - bar_h;
                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.95"/>"##,
                    bar_y, width, bar_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                    bar_y, width, bar_y, palette.badge_bg
                ));

                let cmd_text = format!("{}█", escape_xml(&app_state.modal.command_buffer));
                svg.push_str(&format!(
                    r##"<text x="16" y="{}" fill="{}" font-family="monospace" font-size="14" font-weight="bold">{}</text>"##,
                    bar_y + 23.0, palette.text_main, cmd_text
                ));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="12" text-anchor="end">Enter: Run | Esc: Cancel</text>"##,
                    width as f32 - 16.0, bar_y + 23.0, palette.text_sub
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
            _ => {
                // Normal mode HUD bar at bottom
                let bar_h = 30.0;
                let bar_y = height as f32 - bar_h;
                svg.push_str(&format!(
                    r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.95"/>"##,
                    bar_y, width, bar_h, palette.card_bg
                ));
                svg.push_str(&format!(
                    r##"<line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                    bar_y, width, bar_y, palette.badge_bg
                ));

                let status_line = escape_xml(&app_state.hud_status());
                svg.push_str(&format!(
                    r##"<text x="14" y="{}" fill="{}" font-family="monospace" font-size="12" font-weight="bold">{}</text>"##,
                    bar_y + 19.0, palette.text_main, status_line
                ));

                let hints = ":/& Command | t Test Node | Tab Pivot | a Add | e Edit";
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="11" text-anchor="end">{}</text>"##,
                    width as f32 - 14.0, bar_y + 19.0, palette.text_sub, hints
                ));
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
        if let Some(ref rx) = self.ipc_rx {
            let mut got_cmd = false;
            while let Ok(cmd) = rx.try_recv() {
                self.app_state.handle_ipc_command(cmd);
                got_cmd = true;
            }
            if got_cmd {
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
        }
    }
}
