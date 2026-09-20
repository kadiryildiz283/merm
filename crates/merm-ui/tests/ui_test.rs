use merm_core::LayoutDirection;
use merm_ipc::{CursorMovedParams, EditorCommand, ReloadParams};
use merm_ui::{AppState, ModalController, UiAction};

#[test]
fn test_modal_controller_keybindings() {
    let mut ctrl = ModalController::default();
    assert_eq!(ctrl.handle_key('h'), UiAction::Pan { dx: 30.0, dy: 0.0 });
    assert_eq!(ctrl.handle_key('+'), UiAction::Zoom { factor: 1.15 });
    assert_eq!(ctrl.handle_key('p'), UiAction::PivotDirection);
    assert_eq!(ctrl.handle_key('q'), UiAction::Quit);
}

#[test]
fn test_app_state_ipc_integration() {
    let source = "flowchart TD\n    NodeA[Service A] --> NodeB[Service B]".to_string();
    let mut app = AppState::new(source, false);

    // Simulate cursor moved over Service B
    let cmd = EditorCommand::CursorMoved(CursorMovedParams {
        file: "service.rs".to_string(),
        line: 12,
        column: 4,
        symbol: Some("NodeB".to_string()),
    });
    app.handle_ipc_command(cmd);
    assert_eq!(app.active_node_id.as_deref(), Some("NodeB"));

    // Simulate reload from editor
    let new_source = "flowchart TD\n    X[New Service] --> Y[Database]".to_string();
    let reload_cmd = EditorCommand::Reload(ReloadParams {
        file: "service.rs".to_string(),
        content: Some(new_source),
    });
    app.handle_ipc_command(reload_cmd);
    assert!(app.diagram_source.contains("New Service"));
}

#[test]
fn test_app_state_direction_pivoting() {
    let source = "flowchart TD\n    A --> B".to_string();
    let mut app = AppState::new(source, false);
    assert_eq!(app.active_direction, LayoutDirection::TD);

    app.handle_key_action(UiAction::PivotDirection);
    assert_eq!(app.active_direction, LayoutDirection::LR);
    assert!(app.diagram_source.contains("flowchart LR"));
}
