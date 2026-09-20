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

use crate::app_state::AppState;
use crate::modal::UiAction;

fn parse_hex_color(hex: &str) -> u32 {
    let clean = hex.trim_start_matches('#');
    u32::from_str_radix(clean, 16).unwrap_or(0x1e1e2e)
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
        let hud_color = parse_hex_color(&self.app_state.theme.palette().badge_bg);
        let hud_height = 28u32;

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

        // Always fill background first so the window is never pitch-black
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

        // Draw HUD bar at the bottom: 28px height with current theme color
        if height > hud_height {
            let start_idx = ((height - hud_height) * width) as usize;
            if start_idx < buffer.len() {
                buffer[start_idx..].fill(hud_color);
            }
        }

        buffer.present().expect("Failed to present buffer");
        self.last_frame = Instant::now();
    }
}

impl ApplicationHandler for MermAppWindow {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let initial_title = format!("merm | {}", self.app_state.hud_status());
            let win_attr = Window::default_attributes()
                .with_title(initial_title)
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

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
                let action = match logical_key {
                    Key::Character(c) => self
                        .app_state
                        .modal
                        .handle_key(c.chars().next().unwrap_or(' ')),
                    Key::Named(NamedKey::Escape) => UiAction::Quit,
                    Key::Named(NamedKey::Tab) => UiAction::PivotDirection,
                    _ => UiAction::None,
                };

                match action {
                    UiAction::Quit => {
                        event_loop.exit();
                    }
                    UiAction::None => {}
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
                                    "Class: {} ({} vars, {} methods) | // {}",
                                    node_id, attr_count, meth_count, comm
                                );
                            } else {
                                self.app_state.status_message = format!(
                                    "Class: {} ({} vars, {} methods) | Drag to move",
                                    node_id, attr_count, meth_count
                                );
                            }
                        } else {
                            self.app_state.status_message =
                                format!("Selected: [{}] (drag with mouse)", node_label);
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
        // If there are incoming IPC messages from the editor, process them and request redraw
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
