use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Instant;

use merm_ipc::EditorCommand;
use merm_render::SvgRasterizer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::app_state::{AppState, SidebarTab};
use crate::modal::{UiAction, UiMode};
use crate::studio::StudioOverlay;
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
    pending_drag_pos: Option<(usize, f32, f32)>,
    current_surface_size: (u32, u32),
    mouse_press_pos: Option<(f64, f64)>,
    mouse_press_hit_idx: Option<usize>,
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
            pending_drag_pos: None,
            current_surface_size: (0, 0),
            mouse_press_pos: None,
            mouse_press_hit_idx: None,
        }
    }

    pub fn request_redraw(&self) {
        if let Some(ref w) = self.window {
            w.request_redraw();
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
        let frame_start = self.app_state.telemetry.begin_frame();

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

        let update_start = Instant::now();
        // Drain any incoming IPC commands from Neovim / Helix
        if let Some(ref rx) = self.ipc_rx {
            while let Ok(cmd) = rx.try_recv() {
                self.app_state.handle_ipc_command(cmd);
            }
        }
        let update_ms = update_start.elapsed().as_secs_f32() * 1000.0;

        let bg_color = parse_hex_color(&self.app_state.theme.palette().background);

        let ui_scale = if width >= 2500 || height >= 1500 {
            1.85f32
        } else if width >= 1800 || height >= 1000 {
            1.35f32
        } else {
            1.0f32
        };
        let sidebar_w = if self.app_state.show_left_sidebar {
            210.0 * ui_scale
        } else {
            0.0
        };
        let inspector_w = if self.app_state.show_right_panel {
            300.0 * ui_scale
        } else {
            0.0
        };
        let top_h = 36.0 * ui_scale;
        let bottom_status_h = 0.0;

        if !self.initial_fit_done {
            if let Some(ref diagram) = self.app_state.current_diagram {
                let avail_w = (width as f32 - sidebar_w - inspector_w).max(200.0);
                let avail_h = (height as f32 - top_h - bottom_status_h).max(200.0);
                self.app_state.transform.fit_to_viewport(
                    diagram.width,
                    diagram.height,
                    avail_w,
                    avail_h,
                );
                self.app_state.transform.pan_x += sidebar_w;
                self.app_state.transform.pan_y += top_h;
                self.initial_fit_done = true;
            }
        }

        let render_start = Instant::now();
        // Rasterize active diagram SVG using tiny-skia + resvg (only when on Diagrams tab)
        if self.app_state.active_sidebar_tab == SidebarTab::Diagrams {
            if let Some(ref diagram) = self.app_state.current_diagram {
                match self.rasterizer.rasterize(
                    &diagram.svg,
                    &self.app_state.transform,
                    width,
                    height,
                    &mut buffer,
                    Some(bg_color),
                ) {
                    Ok(cached) => {
                        if cached {
                            self.app_state.telemetry.record_cache_hit();
                        } else {
                            self.app_state.telemetry.record_cache_miss();
                        }
                    }
                    Err(e) => {
                        log::error!("Rasterization failed: {}", e);
                        buffer.fill(bg_color);
                    }
                }
            } else {
                buffer.fill(bg_color);
            }
        } else {
            buffer.fill(bg_color);
        }

        // Render vector UI Overlay (Studio 4-panel system, HUD, drawers, modals)
        if let Some(overlay_svg) = Self::build_overlay_svg(&self.app_state, width, height) {
            if let Err(e) =
                self.rasterizer
                    .rasterize_overlay(&overlay_svg, width, height, &mut buffer)
            {
                log::warn!("Overlay rasterization warning: {}", e);
            }
        }

        buffer.present().expect("Failed to present buffer");
        let render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
        self.last_frame = Instant::now();

        let (visible_nodes, total_nodes) = if let Some(ref diag) = self.app_state.current_diagram {
            let total = diag.nodes.len();
            let pad = 20.0;
            let visible = diag
                .nodes
                .iter()
                .filter(|n| {
                    let (sx, sy) = self.app_state.transform.world_to_screen(n.x, n.y);
                    let sw = n.width * self.app_state.transform.scale;
                    let sh = n.height * self.app_state.transform.scale;
                    sx + sw >= -pad
                        && sx <= width as f32 + pad
                        && sy + sh >= -pad
                        && sy <= height as f32 + pad
                })
                .count();
            (visible, total)
        } else {
            (0, 0)
        };

        self.app_state.telemetry.record_frame(
            frame_start,
            update_ms,
            render_ms,
            visible_nodes,
            total_nodes,
        );
    }

    pub fn build_overlay_svg(app_state: &AppState, width: u32, height: u32) -> Option<String> {
        Some(crate::studio::StudioOverlay::build(
            app_state, width, height,
        ))
    }
}

impl ApplicationHandler for MermAppWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let initial_title = self.app_state.hud_status();
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
                // A. Command Palette Key Interception
                if self.app_state.command_palette_visible
                    || self.app_state.modal.mode == UiMode::Command
                {
                    let filtered = StudioOverlay::filter_palette_items(&self.app_state);
                    match logical_key {
                        Key::Named(NamedKey::ArrowDown) => {
                            if !filtered.is_empty() {
                                self.app_state.command_palette_selected_idx =
                                    (self.app_state.command_palette_selected_idx + 1)
                                        % filtered.len();
                            }
                            self.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            if !filtered.is_empty() {
                                if self.app_state.command_palette_selected_idx == 0 {
                                    self.app_state.command_palette_selected_idx =
                                        filtered.len() - 1;
                                } else {
                                    self.app_state.command_palette_selected_idx -= 1;
                                }
                            }
                            self.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::Enter) => {
                            if !filtered.is_empty() {
                                let idx = self
                                    .app_state
                                    .command_palette_selected_idx
                                    .min(filtered.len() - 1);
                                let action = filtered[idx].action.clone();
                                self.app_state.close_command_palette();
                                StudioOverlay::execute_palette_action(&mut self.app_state, action);
                            } else {
                                self.app_state.close_command_palette();
                            }
                            self.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::Escape) => {
                            self.app_state.close_command_palette();
                            self.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::Backspace) | Key::Named(NamedKey::Delete) => {
                            self.app_state.modal.command_buffer.pop();
                            self.app_state.command_palette_query =
                                self.app_state.modal.command_buffer.clone();
                            self.app_state.command_palette_selected_idx = 0;
                            self.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::Space) => {
                            self.app_state.modal.command_buffer.push(' ');
                            self.app_state.command_palette_query =
                                self.app_state.modal.command_buffer.clone();
                            self.app_state.command_palette_selected_idx = 0;
                            self.request_redraw();
                            return;
                        }
                        Key::Character(c) => {
                            let mut changed = false;
                            for ch in c.chars() {
                                if !ch.is_control() {
                                    self.app_state.modal.command_buffer.push(ch);
                                    changed = true;
                                }
                            }
                            if changed {
                                self.app_state.command_palette_query =
                                    self.app_state.modal.command_buffer.clone();
                                self.app_state.command_palette_selected_idx = 0;
                            }
                            self.request_redraw();
                            return;
                        }
                        _ => {
                            return;
                        }
                    }
                }

                // B. Global Shortcuts (when Command Palette is closed)
                if let Key::Character(ref c) = logical_key {
                    if c == "\x0b" || c == "\x10" {
                        // Ctrl+K or Ctrl+P: open command palette
                        self.app_state.open_command_palette();
                        self.request_redraw();
                        return;
                    }
                    if self.app_state.modal.mode == UiMode::Normal {
                        match c.as_str() {
                            "1" => {
                                self.app_state.set_sidebar_tab(SidebarTab::Diagrams);
                                self.request_redraw();
                                return;
                            }
                            "2" => {
                                self.app_state.set_sidebar_tab(SidebarTab::Explorer);
                                self.request_redraw();
                                return;
                            }
                            "3" => {
                                self.app_state.set_sidebar_tab(SidebarTab::AstView);
                                self.request_redraw();
                                return;
                            }
                            "4" => {
                                self.app_state.set_sidebar_tab(SidebarTab::Executions);
                                self.request_redraw();
                                return;
                            }
                            "5" | "," => {
                                self.app_state.set_sidebar_tab(SidebarTab::Settings);
                                self.request_redraw();
                                return;
                            }
                            _ => {}
                        }
                    }
                }

                let has_selected = self.app_state.active_node_id.is_some();
                let action = match logical_key {
                    Key::Character(c) => {
                        if c == "\x03" {
                            // Ctrl+C: copy split buffer to clipboard
                            self.app_state.copy_report_to_clipboard();
                            UiAction::None
                        } else if c == "\x17" {
                            // Ctrl+W: toggle split buffer
                            self.app_state.execute_command_str(":split");
                            UiAction::None
                        } else {
                            let ch = c.chars().next().unwrap_or(' ');
                            self.app_state.modal.handle_key(ch, has_selected)
                        }
                    }
                    Key::Named(NamedKey::Space) => {
                        self.app_state.modal.handle_key(' ', has_selected)
                    }
                    Key::Named(NamedKey::Backspace) => self.app_state.modal.handle_backspace(),
                    Key::Named(NamedKey::Delete) => self.app_state.modal.handle_backspace(),
                    Key::Named(NamedKey::Enter) => {
                        self.app_state.modal.handle_key('\n', has_selected)
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        if self.app_state.modal.mode != UiMode::Command
                            && self.app_state.modal.mode != UiMode::Search
                            && self.app_state.modal.mode != UiMode::NodeEdit
                            && self.app_state.modal.mode != UiMode::NodeTest
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            self.app_state.select_directional_node(1.0, 0.0);
                            UiAction::None
                        } else {
                            UiAction::None
                        }
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        if self.app_state.modal.mode != UiMode::Command
                            && self.app_state.modal.mode != UiMode::Search
                            && self.app_state.modal.mode != UiMode::NodeEdit
                            && self.app_state.modal.mode != UiMode::NodeTest
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            self.app_state.select_directional_node(-1.0, 0.0);
                            UiAction::None
                        } else {
                            UiAction::None
                        }
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        if self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_add(1);
                            UiAction::None
                        } else if self.app_state.modal.mode != UiMode::Command
                            && self.app_state.modal.mode != UiMode::Search
                            && self.app_state.modal.mode != UiMode::NodeEdit
                            && self.app_state.modal.mode != UiMode::NodeTest
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            self.app_state.select_directional_node(0.0, 1.0);
                            UiAction::None
                        } else {
                            UiAction::None
                        }
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        if self.app_state.modal.mode == UiMode::Report {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_sub(1);
                            UiAction::None
                        } else if self.app_state.modal.mode != UiMode::Command
                            && self.app_state.modal.mode != UiMode::Search
                            && self.app_state.modal.mode != UiMode::NodeEdit
                            && self.app_state.modal.mode != UiMode::NodeTest
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            self.app_state.select_directional_node(0.0, -1.0);
                            UiAction::None
                        } else {
                            UiAction::None
                        }
                    }
                    Key::Named(NamedKey::PageDown) => {
                        if self.app_state.show_split_buffer
                            || self.app_state.modal.mode == UiMode::Report
                        {
                            self.app_state.modal.report_scroll_offset =
                                self.app_state.modal.report_scroll_offset.saturating_add(10);
                            UiAction::None
                        } else {
                            UiAction::Zoom { factor: 0.85 }
                        }
                    }
                    Key::Named(NamedKey::PageUp) => {
                        if self.app_state.show_split_buffer
                            || self.app_state.modal.mode == UiMode::Report
                        {
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
                        if self.app_state.modal.mode != UiMode::Command
                            && self.app_state.modal.mode != UiMode::Search
                            && self.app_state.modal.mode != UiMode::NodeEdit
                            && self.app_state.modal.mode != UiMode::NodeTest
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            UiAction::SelectNextNode
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
                            let title = self.app_state.hud_status();
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
                            let title = self.app_state.hud_status();
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
                let split_top_y = (win_h as f64 * 0.48).clamp(
                    240.0 * ui_scale,
                    (win_h as f64 - bottom_bars_h - 40.0).max(120.0),
                );
                let is_split_open =
                    self.app_state.show_split_buffer || self.app_state.modal.mode == UiMode::Report;
                if is_split_open && cy >= (win_h as f64 - split_top_y) {
                    let scroll_delta: i32 = match delta {
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
                    self.pending_drag_pos = Some((idx, new_x, new_y));
                    self.app_state.move_node_preview(idx, new_x, new_y);

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
                let (cx, cy) = self.last_cursor_pos.unwrap_or((640.0, 360.0));
                let (win_w, win_h) = self.current_surface_size;
                let ui_scale = if win_w >= 2500 || win_h >= 1500 {
                    1.85f64
                } else if win_w >= 1800 || win_h >= 1000 {
                    1.35f64
                } else {
                    1.0f64
                };

                if state == ElementState::Pressed {
                    self.mouse_press_pos = Some((cx, cy));

                    // 1. If in NodeEdit mode, handle clicks on buttons or backdrop
                    if self.app_state.modal.mode == UiMode::NodeEdit {
                        let w = win_w as f64;
                        let h = win_h as f64;
                        let modal_w =
                            (w - 140.0 * ui_scale).clamp(520.0 * ui_scale, 860.0 * ui_scale);
                        let modal_h =
                            (h - 100.0 * ui_scale).clamp(380.0 * ui_scale, 620.0 * ui_scale);
                        let modal_x = (w - modal_w) / 2.0;
                        let modal_y = (h - modal_h) / 2.0;
                        let header_h = 38.0 * ui_scale;

                        // Click outside modal card -> close modal
                        if cx < modal_x
                            || cx > modal_x + modal_w
                            || cy < modal_y
                            || cy > modal_y + modal_h
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            if let Some(ref w) = self.window {
                                w.request_redraw();
                            }
                            return;
                        }

                        // Click on [Esc] Cancel button in header
                        if cy >= modal_y
                            && cy <= modal_y + header_h
                            && cx >= modal_x + modal_w - 95.0 * ui_scale
                        {
                            self.app_state.modal.mode = UiMode::Normal;
                            if let Some(ref w) = self.window {
                                w.request_redraw();
                            }
                            return;
                        }

                        // Click on [Enter] Add/Save button in header
                        if cy >= modal_y
                            && cy <= modal_y + header_h
                            && cx >= modal_x + modal_w - 210.0 * ui_scale
                        {
                            let action = self.app_state.modal.handle_key('\n', true);
                            self.app_state.handle_key_action(action);
                            if let Some(ref w) = self.window {
                                w.request_redraw();
                            }
                            return;
                        }

                        return;
                    }

                    // 2. Click inside persistent Split Buffer area
                    let is_split_open = self.app_state.show_split_buffer
                        || self.app_state.modal.mode == UiMode::Report;
                    let bottom_bars_h = (24.0 + 28.0) * ui_scale;
                    let split_h = (win_h as f64 * 0.48).clamp(
                        240.0 * ui_scale,
                        (win_h as f64 - bottom_bars_h - 40.0).max(120.0),
                    );
                    let split_y = win_h as f64 - split_h;
                    let split_header_h = 26.0 * ui_scale;

                    if is_split_open && cy >= split_y && cy < (win_h as f64 - bottom_bars_h) {
                        let w = win_w as f64;
                        // Click in Split Header: buttons
                        if cy <= split_y + split_header_h {
                            // [📋 Kopyala] button: cx between w - 245.0 * ui_scale and w - 150.0 * ui_scale
                            if cx >= w - 245.0 * ui_scale && cx <= w - 150.0 * ui_scale {
                                self.app_state.copy_report_to_clipboard();
                                if let Some(ref w) = self.window {
                                    let title = self.app_state.hud_status();
                                    w.set_title(&title);
                                    w.request_redraw();
                                }
                                return;
                            }
                            // [▲] button: cx between w - 145.0 * ui_scale and w - 115.0 * ui_scale
                            if cx >= w - 145.0 * ui_scale && cx <= w - 115.0 * ui_scale {
                                self.app_state.modal.mode = UiMode::Report;
                                self.app_state.modal.report_scroll_offset =
                                    self.app_state.modal.report_scroll_offset.saturating_sub(5);
                                if let Some(ref w) = self.window {
                                    w.request_redraw();
                                }
                                return;
                            }
                            // [▼] button: cx between w - 110.0 * ui_scale and w - 80.0 * ui_scale
                            if cx >= w - 110.0 * ui_scale && cx <= w - 80.0 * ui_scale {
                                self.app_state.modal.mode = UiMode::Report;
                                self.app_state.modal.report_scroll_offset =
                                    self.app_state.modal.report_scroll_offset.saturating_add(5);
                                if let Some(ref w) = self.window {
                                    w.request_redraw();
                                }
                                return;
                            }
                            // [✕] button: cx between w - 75.0 * ui_scale and w - 20.0 * ui_scale
                            if cx >= w - 75.0 * ui_scale {
                                self.app_state.show_split_buffer = false;
                                self.app_state.modal.mode = UiMode::Normal;
                                self.app_state.status_message =
                                    "Split buffer closed (maximized diagram)".to_string();
                                if let Some(ref w) = self.window {
                                    let title = self.app_state.hud_status();
                                    w.set_title(&title);
                                    w.request_redraw();
                                }
                                return;
                            }
                        }

                        // Click in Split Body: focus split buffer & select clicked line
                        self.app_state.modal.mode = UiMode::Report;
                        let content_top_y = split_y + split_header_h + 8.0 * ui_scale;
                        let line_h = 18.0 * ui_scale;
                        let clicked_line = if cy >= content_top_y {
                            self.app_state.modal.report_scroll_offset
                                + ((cy - content_top_y) / line_h).floor() as usize
                        } else {
                            self.app_state.modal.report_scroll_offset
                        };
                        self.app_state.status_message = format!(
                            "Split Buffer: Satır {} odaklandı. 'y' veya [📋 Kopyala] ile panoya kopyalayabilirsiniz.",
                            clicked_line + 1
                        );
                        if let Some(ref w) = self.window {
                            let title = self.app_state.hud_status();
                            w.set_title(&title);
                            w.request_redraw();
                        }
                        return;
                    }

                    // 3. Studio 4-Panel Overlay Hit Testing (Top bar, Sidebars, Modals, Tools, Action Pills)
                    if let Some(action) = crate::studio::StudioHitTester::handle_mouse_click(
                        &mut self.app_state,
                        cx,
                        cy,
                        win_w,
                        win_h,
                    ) {
                        if action == UiAction::Quit {
                            event_loop.exit();
                            return;
                        }
                        self.app_state.handle_key_action(action);
                        if let Some(ref w) = self.window {
                            let title = self.app_state.hud_status();
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
                        let pad = 6.0;
                        diag.nodes.iter().position(|n| {
                            world_x >= n.x - pad
                                && world_x <= n.x + n.width + pad
                                && world_y >= n.y - pad
                                && world_y <= n.y + n.height + pad
                        })
                    } else {
                        None
                    };

                    self.mouse_press_hit_idx = hit_idx;

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
                                    "Class: {} ({} vars, {} methods) | // {} | Click to edit",
                                    node_id, attr_count, meth_count, comm
                                );
                            } else {
                                self.app_state.status_message = format!(
                                    "Class: {} ({} vars, {} methods) | Click to edit | Drag to move",
                                    node_id, attr_count, meth_count
                                );
                            }
                        } else {
                            self.app_state.status_message = format!(
                                "Selected: [{}] (click to edit | drag to move)",
                                node_label
                            );
                        }
                        if let Some(ref w) = self.window {
                            let title = self.app_state.hud_status();
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    } else {
                        self.dragging_node_idx = None;
                        self.mouse_dragging = true;
                        self.app_state.select_node(None);
                        if let Some(ref w) = self.window {
                            let title = self.app_state.hud_status();
                            w.set_title(&title);
                            w.request_redraw();
                        }
                    }
                } else {
                    // ElementState::Released
                    let is_click = if let Some((px, py)) = self.mouse_press_pos.take() {
                        (cx - px).hypot(cy - py) < 6.0
                    } else {
                        false
                    };

                    if is_click {
                        self.pending_drag_pos = None;
                        self.app_state.active_drag_preview = None;
                        self.mouse_press_hit_idx = None;
                    } else {
                        // Was a drag move: commit final position to diagram & record undo delta
                        if let Some((i, nx, ny)) = self.pending_drag_pos.take() {
                            self.app_state.finalize_node_move(i, nx, ny);
                            if let Some(ref w) = self.window {
                                w.request_redraw();
                            }
                        } else {
                            self.app_state.active_drag_preview = None;
                        }
                    }

                    self.mouse_press_hit_idx = None;
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

        // Apply any pending node drag preview position before drawing
        if let Some((i, nx, ny)) = self.pending_drag_pos {
            self.app_state.move_node_preview(i, nx, ny);
            needs_redraw = true;
        }

        if self.app_state.is_busy {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(16),
            ));
            needs_redraw = true;
        } else if self.dragging_node_idx.is_some() || self.mouse_dragging {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(8),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + std::time::Duration::from_millis(32),
            ));
        }

        if !self.app_state.is_running {
            event_loop.exit();
            return;
        }

        if needs_redraw {
            if let Some(ref w) = self.window {
                let title = self.app_state.hud_status();
                w.set_title(&title);
                w.request_redraw();
            }
        }
    }
}
