use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphMutationDelta {
    MoveNode {
        node_id: String,
        old_x: f32,
        old_y: f32,
        new_x: f32,
        new_y: f32,
    },
    DiagramSourceChange {
        source_before: String,
        source_after: String,
        description: String,
    },
}

impl GraphMutationDelta {
    pub fn invert(&self) -> GraphMutationDelta {
        match self {
            GraphMutationDelta::MoveNode {
                node_id,
                old_x,
                old_y,
                new_x,
                new_y,
            } => GraphMutationDelta::MoveNode {
                node_id: node_id.clone(),
                old_x: *new_x,
                old_y: *new_y,
                new_x: *old_x,
                new_y: *old_y,
            },
            GraphMutationDelta::DiagramSourceChange {
                source_before,
                source_after,
                description,
            } => GraphMutationDelta::DiagramSourceChange {
                source_before: source_after.clone(),
                source_after: source_before.clone(),
                description: format!("Undo: {}", description),
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct UndoRedoStack {
    undo_stack: Vec<GraphMutationDelta>,
    redo_stack: Vec<GraphMutationDelta>,
    max_history: usize,
}

impl UndoRedoStack {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: max_history.max(10),
        }
    }

    pub fn push(&mut self, delta: GraphMutationDelta) {
        if self.undo_stack.len() >= self.max_history {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(delta);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<GraphMutationDelta> {
        let delta = self.undo_stack.pop()?;
        let inverted = delta.invert();
        self.redo_stack.push(delta);
        Some(inverted)
    }

    pub fn redo(&mut self) -> Option<GraphMutationDelta> {
        let delta = self.redo_stack.pop()?;
        self.undo_stack.push(delta.clone());
        Some(delta)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_redo_move_inversion() {
        let mut stack = UndoRedoStack::new(50);
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());

        let move_delta = GraphMutationDelta::MoveNode {
            node_id: "AuthService".to_string(),
            old_x: 100.0,
            old_y: 200.0,
            new_x: 250.0,
            new_y: 400.0,
        };
        stack.push(move_delta);
        assert!(stack.can_undo());
        assert_eq!(stack.undo_count(), 1);

        let undone = stack.undo().expect("Should undo");
        match undone {
            GraphMutationDelta::MoveNode {
                node_id,
                old_x,
                old_y,
                new_x,
                new_y,
            } => {
                assert_eq!(node_id, "AuthService");
                assert_eq!(old_x, 250.0);
                assert_eq!(old_y, 400.0);
                assert_eq!(new_x, 100.0);
                assert_eq!(new_y, 200.0);
            }
            _ => panic!("Expected MoveNode delta"),
        }

        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        let redone = stack.redo().expect("Should redo");
        match redone {
            GraphMutationDelta::MoveNode {
                node_id,
                old_x,
                old_y,
                new_x,
                new_y,
            } => {
                assert_eq!(node_id, "AuthService");
                assert_eq!(old_x, 100.0);
                assert_eq!(old_y, 200.0);
                assert_eq!(new_x, 250.0);
                assert_eq!(new_y, 400.0);
            }
            _ => panic!("Expected MoveNode delta"),
        }
    }
}
