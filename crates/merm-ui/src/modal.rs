#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Normal,
    Pan,
    Search,
    Jump,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    Pan { dx: f32, dy: f32 },
    Zoom { factor: f32 },
    ResetView,
    PivotDirection,
    SetMode(UiMode),
    SelectNextNode,
    SelectPrevNode,
    Quit,
    None,
}

pub struct ModalController {
    pub mode: UiMode,
    pub search_query: String,
}

impl Default for ModalController {
    fn default() -> Self {
        Self {
            mode: UiMode::Normal,
            search_query: String::new(),
        }
    }
}

impl ModalController {
    pub fn handle_key(&mut self, key: char) -> UiAction {
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
                'n' | '\t' => UiAction::SelectNextNode,
                'N' => UiAction::SelectPrevNode,
                '/' | 'f' => {
                    self.mode = UiMode::Search;
                    self.search_query.clear();
                    UiAction::SetMode(UiMode::Search)
                }
                'q' => UiAction::Quit,
                _ => UiAction::None,
            },
            UiMode::Search => {
                if key == '\x1b' {
                    // Escape
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else if key == '\n' || key == '\r' {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else {
                    self.search_query.push(key);
                    UiAction::None
                }
            }
            UiMode::Pan | UiMode::Jump => {
                if key == '\x1b' {
                    self.mode = UiMode::Normal;
                    UiAction::SetMode(UiMode::Normal)
                } else {
                    UiAction::None
                }
            }
        }
    }
}
