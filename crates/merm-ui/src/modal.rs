#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Normal,
    Pan,
    Search,
    Jump,
    Command,
    NodeTest,
    Report,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    Pan { dx: f32, dy: f32 },
    Zoom { factor: f32 },
    ResetView,
    PivotDirection,
    CycleTheme,
    SetMode(UiMode),
    SelectNextNode,
    SelectPrevNode,
    ExecuteCommand(String),
    ExecuteNodeTest { node_id: String, input: String },
    OpenEditor(String),
    Reload,
    Quit,
    None,
}

pub struct ModalController {
    pub mode: UiMode,
    pub search_query: String,
    pub command_buffer: String,
    pub test_input_buffer: String,
    pub active_test_node_id: Option<String>,
}

impl Default for ModalController {
    fn default() -> Self {
        Self {
            mode: UiMode::Normal,
            search_query: String::new(),
            command_buffer: String::new(),
            test_input_buffer: String::new(),
            active_test_node_id: None,
        }
    }
}

impl ModalController {
    pub fn handle_key(&mut self, key: char, has_selected_node: bool) -> UiAction {
        match self.mode {
            UiMode::Normal => match key {
                'h' => UiAction::Pan { dx: 30.0, dy: 0.0 },
                'l' => UiAction::Pan { dx: -30.0, dy: 0.0 },
                'k' => UiAction::Pan { dx: 0.0, dy: 30.0 },
                'j' => UiAction::Pan { dx: 0.0, dy: -30.0 },
                '+' | '=' => UiAction::Zoom { factor: 1.15 },
                '-' | '_' => UiAction::Zoom { factor: 0.85 },
                '0' => UiAction::ResetView,
                'p' => UiAction::PivotDirection,
                't' => {
                    if has_selected_node {
                        self.mode = UiMode::NodeTest;
                        UiAction::SetMode(UiMode::NodeTest)
                    } else {
                        UiAction::CycleTheme
                    }
                }
                'T' => UiAction::CycleTheme,
                'n' | '\t' => UiAction::SelectNextNode,
                'N' => UiAction::SelectPrevNode,
                ':' | '&' => {
                    self.mode = UiMode::Command;
                    self.command_buffer.clear();
                    self.command_buffer.push(key);
                    UiAction::SetMode(UiMode::Command)
                }
                'a' | 'o' => {
                    self.mode = UiMode::Command;
                    self.command_buffer = ":add class ".to_string();
                    UiAction::SetMode(UiMode::Command)
                }
                'c' => {
                    self.mode = UiMode::Command;
                    self.command_buffer = ":connect ".to_string();
                    UiAction::SetMode(UiMode::Command)
                }
                'e' => {
                    if has_selected_node {
                        UiAction::OpenEditor(String::new())
                    } else {
                        UiAction::None
                    }
                }
                '/' | 'f' => {
                    self.mode = UiMode::Search;
                    self.search_query.clear();
                    UiAction::SetMode(UiMode::Search)
                }
                'q' => UiAction::Quit,
                _ => UiAction::None,
            },
            UiMode::Command => {
                if key == '\x1b' {
                    // Escape
                    self.mode = UiMode::Normal;
                    self.command_buffer.clear();
                    UiAction::SetMode(UiMode::Normal)
                } else if key == '\n' || key == '\r' {
                    let cmd = self.command_buffer.clone();
                    self.mode = UiMode::Normal;
                    self.command_buffer.clear();
                    UiAction::ExecuteCommand(cmd)
                } else if key == '\x08' {
                    self.handle_backspace()
                } else {
                    self.command_buffer.push(key);
                    UiAction::None
                }
            }
            UiMode::NodeTest => {
                if key == '\x1b' {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else if key == '\n' || key == '\r' {
                    let node_id = self.active_test_node_id.clone().unwrap_or_default();
                    let input = self.test_input_buffer.clone();
                    UiAction::ExecuteNodeTest { node_id, input }
                } else if key == '\x08' {
                    self.handle_backspace()
                } else {
                    self.test_input_buffer.push(key);
                    UiAction::None
                }
            }
            UiMode::Search => {
                if key == '\x1b' || key == '\n' || key == '\r' {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else if key == '\x08' {
                    self.handle_backspace()
                } else {
                    self.search_query.push(key);
                    UiAction::None
                }
            }
            UiMode::Report | UiMode::Pan | UiMode::Jump => {
                if key == '\x1b' || key == 'q' || key == '\n' || key == '\r' {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else {
                    UiAction::None
                }
            }
        }
    }

    pub fn handle_backspace(&mut self) -> UiAction {
        match self.mode {
            UiMode::Command => {
                self.command_buffer.pop();
                if self.command_buffer.is_empty() {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else {
                    UiAction::None
                }
            }
            UiMode::NodeTest => {
                self.test_input_buffer.pop();
                UiAction::None
            }
            UiMode::Search => {
                self.search_query.pop();
                UiAction::None
            }
            _ => UiAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_command_flow() {
        let mut controller = ModalController::default();
        let action = controller.handle_key(':', false);
        assert_eq!(action, UiAction::SetMode(UiMode::Command));
        assert_eq!(controller.command_buffer, ":");

        controller.handle_key('s', false);
        controller.handle_key('e', false);
        controller.handle_key('t', false);
        assert_eq!(controller.command_buffer, ":set");

        let exec = controller.handle_key('\n', false);
        assert_eq!(exec, UiAction::ExecuteCommand(":set".to_string()));
        assert_eq!(controller.mode, UiMode::Normal);
    }
}
