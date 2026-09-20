use std::time::Duration;
use merm_core::{AstRewriter, DiagramExtractor, LayoutDirection, LayoutEngine, RenderedDiagram, ThemeId};
use merm_ipc::EditorCommand;
use merm_render::{BackendType, RenderEngine, Transform2D};

use crate::modal::{ModalController, UiAction, UiMode};

pub struct AppState {
    pub diagram_source: String,
    pub active_direction: LayoutDirection,
    pub current_diagram: Option<RenderedDiagram>,
    pub transform: Transform2D,
    pub modal: ModalController,
    pub render_engine: RenderEngine,
    pub active_node_id: Option<String>,
    pub theme: ThemeId,
    pub status_message: String,
    pub is_running: bool,
}

impl AppState {
    pub fn new(source: String, force_software_render: bool) -> Self {
        Self::with_theme(source, force_software_render, ThemeId::CatppuccinMocha)
    }

    pub fn with_theme(source: String, force_software_render: bool, theme: ThemeId) -> Self {
        let render_engine = if force_software_render {
            RenderEngine::force_software()
        } else {
            RenderEngine::new_with_auto_fallback()
        };

        let mut state = Self {
            diagram_source: source,
            active_direction: LayoutDirection::TD,
            current_diagram: None,
            transform: Transform2D::default(),
            modal: ModalController::default(),
            render_engine,
            active_node_id: None,
            theme,
            status_message: "Ready".to_string(),
            is_running: true,
        };

        state.recalculate_diagram();
        state
    }

    pub fn recalculate_diagram(&mut self) {
        let engine = LayoutEngine::new(
            Duration::from_millis(2000),
            32 * 1024 * 1024,
            self.theme.palette(),
        );
        let extracted = DiagramExtractor::extract(&self.diagram_source);

        let source_to_render = match extracted {
            Ok(blocks) if !blocks.is_empty() => blocks[0].source.clone(),
            _ => self.diagram_source.clone(),
        };

        match engine.render_with_watchdog(&source_to_render) {
            Ok(diagram) => {
                self.transform.fit_to_viewport(diagram.width, diagram.height, 1280.0, 720.0);
                self.current_diagram = Some(diagram);
                self.status_message = format!(
                    "Loaded diagram (Backend: {:?})",
                    self.render_engine.active_backend()
                );
            }
            Err(e) => {
                self.status_message = format!("Error calculating layout: {}", e);
            }
        }
    }

    pub fn handle_ipc_command(&mut self, command: EditorCommand) {
        match command {
            EditorCommand::CursorMoved(params) => {
                log::debug!("Editor cursor moved: {:?}", params);
                if let Some(sym) = params.symbol {
                    // Match node by ID or label
                    if let Some(ref diag) = self.current_diagram {
                        if let Some(node) = diag.nodes.iter().find(|n| n.id == sym || n.label == sym) {
                            self.active_node_id = Some(node.id.clone());
                            self.status_message = format!("Focused node: {}", node.label);
                        }
                    }
                }
            }
            EditorCommand::Reload(params) => {
                if let Some(content) = params.content {
                    self.diagram_source = content;
                    self.recalculate_diagram();
                    self.status_message = format!("Diagram reloaded from editor ({})", params.file);
                }
            }
            EditorCommand::JumpToDefinition(params) => {
                self.active_node_id = Some(params.node_id.clone());
                self.status_message = format!(
                    "Jump to definition -> {}:{}",
                    params.target_file, params.target_line
                );
            }
            EditorCommand::Ping => {
                // Handled in server
            }
            EditorCommand::Custom { method, .. } => {
                log::debug!("Unhandled custom IPC method: {}", method);
            }
        }
    }

    pub fn handle_key_action(&mut self, action: UiAction) {
        match action {
            UiAction::Pan { dx, dy } => {
                self.transform.pan(dx, dy);
            }
            UiAction::Zoom { factor } => {
                self.transform.zoom_at(factor, 640.0, 360.0);
            }
            UiAction::ResetView => {
                if let Some(ref diag) = self.current_diagram {
                    self.transform.fit_to_viewport(diag.width, diag.height, 1280.0, 720.0);
                }
            }
            UiAction::PivotDirection => {
                let next_dir = self.active_direction.next();
                self.diagram_source =
                    AstRewriter::pivot_direction(&self.diagram_source, next_dir);
                self.active_direction = next_dir;
                self.recalculate_diagram();
                self.status_message = format!("Pivoted direction to {}", next_dir.as_str());
            }
            UiAction::CycleTheme => {
                self.theme = self.theme.next();
                if let Some(ref mut diag) = self.current_diagram {
                    diag.regenerate_svg(&self.theme.palette());
                } else {
                    self.recalculate_diagram();
                }
                self.status_message = format!("Theme: {}", self.theme.palette().name);
            }
            UiAction::SelectNextNode => {
                let next_info = self.current_diagram.as_ref().and_then(|diag| {
                    if diag.nodes.is_empty() {
                        None
                    } else {
                        let cur_idx = self
                            .active_node_id
                            .as_ref()
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id))
                            .unwrap_or(0);
                        let next_idx = (cur_idx + 1) % diag.nodes.len();
                        Some((diag.nodes[next_idx].id.clone(), diag.nodes[next_idx].label.clone()))
                    }
                });
                if let Some((next_id, next_label)) = next_info {
                    self.select_node(Some(&next_id));
                    self.status_message = format!("Selected: {}", next_label);
                }
            }
            UiAction::SelectPrevNode => {
                let prev_info = self.current_diagram.as_ref().and_then(|diag| {
                    if diag.nodes.is_empty() {
                        None
                    } else {
                        let cur_idx = self
                            .active_node_id
                            .as_ref()
                            .and_then(|id| diag.nodes.iter().position(|n| &n.id == id))
                            .unwrap_or(0);
                        let prev_idx = if cur_idx == 0 {
                            diag.nodes.len() - 1
                        } else {
                            cur_idx - 1
                        };
                        Some((diag.nodes[prev_idx].id.clone(), diag.nodes[prev_idx].label.clone()))
                    }
                });
                if let Some((prev_id, prev_label)) = prev_info {
                    self.select_node(Some(&prev_id));
                    self.status_message = format!("Selected: {}", prev_label);
                }
            }
            UiAction::Quit => {
                self.is_running = false;
            }
            UiAction::SetMode(_) | UiAction::None => {}
        }
    }

    pub fn move_node(&mut self, node_idx: usize, new_x: f32, new_y: f32) {
        if let Some(ref mut diag) = self.current_diagram {
            if node_idx < diag.nodes.len() {
                diag.nodes[node_idx].x = new_x;
                diag.nodes[node_idx].y = new_y;
                diag.regenerate_svg(&self.theme.palette());
            }
        }
    }

    pub fn select_node(&mut self, node_id: Option<&str>) {
        if let Some(ref mut diag) = self.current_diagram {
            diag.selected_node_id = node_id.map(|s| s.to_string());
            diag.regenerate_svg(&self.theme.palette());
        }
        self.active_node_id = node_id.map(|s| s.to_string());
    }

    pub fn render_current_frame(&mut self) -> Option<merm_render::RenderResult> {
        if let Some(ref diag) = self.current_diagram {
            let res = self.render_engine.render(&self.transform, &diag.svg);
            Some(res)
        } else {
            None
        }
    }

    pub fn hud_status(&self) -> String {
        let backend_str = match self.render_engine.active_backend() {
            BackendType::HardwareWgpu => "WGPU 120FPS",
            BackendType::SoftwareFallback => "CPU (softbuffer) 60FPS",
        };
        let mode_str = match self.modal.mode {
            UiMode::Normal => "NORMAL",
            UiMode::Pan => "PAN",
            UiMode::Search => "SEARCH",
            UiMode::Jump => "JUMP",
        };

        let sel_str = if let Some(ref sel_id) = self.active_node_id {
            if let Some(ref diag) = self.current_diagram {
                if let Some(node) = diag.nodes.iter().find(|n| &n.id == sel_id) {
                    if !node.attributes.is_empty() || !node.methods.is_empty() || node.stereotype.is_some() || node.doc_comment.is_some() {
                        let type_kind = node.stereotype.as_deref().unwrap_or("CLASS");
                        format!(" [{}: {} ({} vars, {} funcs)]", type_kind.to_uppercase(), node.id, node.attributes.len(), node.methods.len())
                    } else {
                        format!(" [NODE: {}]", node.label)
                    }
                } else {
                    format!(" [{}]", sel_id)
                }
            } else {
                format!(" [{}]", sel_id)
            }
        } else {
            String::new()
        };

        format!(
            "[MODE: {}] [{}] [{}] [Zoom: {:.1}x]{} | {}",
            mode_str, self.theme.palette().name, backend_str, self.transform.scale, sel_str, self.status_message
        )
    }
}
