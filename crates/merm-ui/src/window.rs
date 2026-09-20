use std::sync::Arc;
use std::sync::mpsc::Receiver;
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
        }
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
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

        surface
            .resize(
                std::num::NonZeroU32::new(width).unwrap(),
                std::num::NonZeroU32::new(height).unwrap(),
            )
            .expect("Failed to resize softbuffer surface");

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

        // Rasterize active diagram SVG using tiny-skia + resvg
        if let Some(ref diagram) = self.app_state.current_diagram {
            let _ = self.rasterizer.rasterize(
                &diagram.svg,
                &self.app_state.transform,
                width,
                height,
                &mut buffer,
            );
        } else {
            buffer.fill(bg_color);
        }

        // Draw HUD bar at the bottom: 28px height with current theme color
        let hud_height = 28u32;
        if height > hud_height {
            let start_y = height - hud_height;
            for y in start_y..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    if idx < buffer.len() {
                        buffer[idx] = hud_color;
                    }
                }
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
                    Key::Character(c) => self.app_state.modal.handle_key(c.chars().next().unwrap_or(' ')),
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
                self.app_state.transform.zoom_at(factor, cx as f32, cy as f32);
                if let Some(ref w) = self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.mouse_dragging {
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
                self.mouse_dragging = state == ElementState::Pressed;
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
