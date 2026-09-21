use merm_core::LayoutDirection;
use merm_ipc::{CursorMovedParams, EditorCommand, ReloadParams};
use merm_ui::{AppState, ModalController, UiAction};

#[test]
fn test_modal_controller_keybindings() {
    let mut ctrl = ModalController::default();
    assert_eq!(
        ctrl.handle_key('h', false),
        UiAction::Pan { dx: 30.0, dy: 0.0 }
    );
    assert_eq!(ctrl.handle_key('+', false), UiAction::Zoom { factor: 1.15 });
    assert_eq!(ctrl.handle_key('p', false), UiAction::PivotDirection);
    assert_eq!(ctrl.handle_key('q', false), UiAction::Quit);

    // Command mode trigger
    assert_eq!(
        ctrl.handle_key(':', false),
        UiAction::SetMode(merm_ui::UiMode::Command)
    );
    assert_eq!(ctrl.command_buffer, ":");
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

#[test]
fn test_app_state_command_dispatch() {
    let source = "classDiagram\n    class User\n".to_string();
    let mut app = AppState::new(source, false);

    // Help command
    app.execute_command_str(":help");
    assert_eq!(app.modal.mode, merm_ui::UiMode::Report);
    assert!(app
        .report_content
        .as_ref()
        .unwrap()
        .contains("Interactive Command Protocol"));

    // Add node command
    app.manifest = None;
    app.execute_command_str(":add class BillingService");
    assert!(app.diagram_source.contains("class BillingService"));
    assert!(app.status_message.contains("Added node 'BillingService'"));

    // Connect command
    app.execute_command_str(":connect User BillingService pays");
    assert!(app
        .diagram_source
        .contains("User --> BillingService : pays"));

    // Copy command
    app.show_report("Architecture Report Content\nLine 2".to_string());
    app.execute_command_str(":copy");
    assert!(
        app.status_message.contains("Copied")
            || app.status_message.contains("Split")
            || app.status_message.contains("clipboard")
    );

    // Test Tab auto-complete in command mode
    let mut ctrl = ModalController {
        mode: merm_ui::UiMode::Command,
        command_buffer: "&advice".to_string(),
        ..Default::default()
    };
    ctrl.handle_key('\t', false);
    assert!(ctrl.command_buffer.contains("Analyze architecture"));
}

#[test]
fn test_app_state_inspector_mode() {
    let source = "classDiagram\n    class AuthService\n".to_string();
    let mut app = AppState::new(source, false);
    app.select_node(Some("AuthService"));

    // Trigger inspector
    let act = app.modal.handle_key('i', true);
    assert_eq!(act, UiAction::SetMode(merm_ui::UiMode::Inspector));
    app.handle_key_action(act);

    assert_eq!(app.modal.mode, merm_ui::UiMode::Inspector);
    let hud = app.hud_status();
    assert!(hud.contains("[INSPECTOR: AuthService]"));

    // Exit inspector
    let act_close = app.modal.handle_key('q', true);
    assert_eq!(act_close, UiAction::SetMode(merm_ui::UiMode::Normal));
    app.handle_key_action(act_close);
    assert_eq!(app.modal.mode, merm_ui::UiMode::Normal);
}

#[test]
fn test_app_state_worker_async_result_handling() {
    let source = "classDiagram\n    class WorkerNode\n".to_string();
    let mut app = AppState::new(source, false);

    // Simulate worker sending a test result
    let worker_res = merm_ui::WorkerResult::TestFinished {
        node_id: "WorkerNode".to_string(),
        result: Ok(merm_core::ExecutionResult {
            success: true,
            output_payload: "{\"status\": \"healthy\"}".to_string(),
            duration_ms: 15,
            exit_code: Some(0),
            stdout: "health check ok".to_string(),
            stderr: String::new(),
        }),
    };

    app.is_busy = true;
    app.busy_message = "Testing...".to_string();

    app.handle_worker_result(worker_res);

    assert!(!app.is_busy);
    assert!(app
        .status_message
        .contains("Node 'WorkerNode' [SUCCESS] (15ms)"));
    assert!(app.last_execution_result.is_some());
    assert!(app
        .report_content
        .as_ref()
        .unwrap()
        .contains("=== Node Execution Result: WorkerNode ==="));
}

#[test]
fn test_overlay_svg_validity_and_rasterization() {
    let source = "classDiagram\n    class User\n".to_string();
    let app = AppState::new(source, false);
    let overlay_svg = merm_ui::MermAppWindow::build_overlay_svg(&app, 1280, 720).unwrap();

    // Verify raw unescaped '&' is eliminated
    assert!(!overlay_svg.contains("<text>&check</text>"));
    assert!(!overlay_svg.contains("<text>&ai</text>"));
    assert!(overlay_svg.contains("&amp;check"));

    // Verify SVG parses and rasterizes with zero errors!
    let mut buffer = vec![0u32; 1280 * 720];
    let rasterizer = merm_render::SvgRasterizer::new();
    let res = rasterizer.rasterize_overlay(&overlay_svg, 1280, 720, &mut buffer);
    assert!(
        res.is_ok(),
        "Overlay SVG must parse and rasterize cleanly: {:?}",
        res.err()
    );
}

#[test]
fn test_overlay_svg_report_mode_rasterization() {
    let source = "classDiagram\n    class Architecture\n".to_string();
    let mut app = AppState::new(source, false);

    // Create a 60-line architectural report with markdown, status, and code blocks
    let mut report =
        String::from("# Comprehensive Architecture Audit\n\n=== Analysis Results ===\n");
    for i in 1..=50 {
        report.push_str(&format!(
            "* Item {}: Status: PASS (Node verified successfully)\n",
            i
        ));
    }
    report.push_str("```mermaid\nclassDiagram\n  class NewService\n```\n");

    app.show_report(report);
    assert_eq!(app.modal.mode, merm_ui::UiMode::Report);
    assert_eq!(app.modal.report_scroll_offset, 18);

    // Generate overlay SVG in Report mode
    let overlay_svg = merm_ui::MermAppWindow::build_overlay_svg(&app, 1280, 720).unwrap();
    assert!(overlay_svg.contains("AI Architecture &amp; Diagnostic Buffer"));
    assert!(overlay_svg.contains("[AI Chat Buffer]"));

    // Rasterize overlay to verify no SVG parsing errors
    let mut buffer = vec![0u32; 1280 * 720];
    let rasterizer = merm_render::SvgRasterizer::new();
    let res = rasterizer.rasterize_overlay(&overlay_svg, 1280, 720, &mut buffer);
    assert!(
        res.is_ok(),
        "Report mode split buffer SVG must parse and rasterize without errors: {:?}",
        res.err()
    );

    // Test scrolling in Report mode
    app.modal.report_scroll_offset = 25;
    let scrolled_svg = merm_ui::MermAppWindow::build_overlay_svg(&app, 1280, 720).unwrap();
    let res_scrolled = rasterizer.rasterize_overlay(&scrolled_svg, 1280, 720, &mut buffer);
    assert!(res_scrolled.is_ok());
}

#[test]
fn test_vim_commands_execution() {
    let source = "classDiagram\n    class Service\n".to_string();
    let mut app = AppState::new(source, false);

    // 1. :theme command
    app.execute_command_str(":theme dracula");
    assert_eq!(app.theme, merm_core::ThemeId::Dracula);

    // 2. :dir command
    app.execute_command_str(":dir LR");
    assert_eq!(app.active_direction, merm_core::LayoutDirection::LR);

    // 3. :fit command
    app.execute_command_str(":fit");
    assert!(app.status_message.contains("Diagram view fitted"));

    // 4. :reset command
    app.execute_command_str(":reset");
    assert_eq!(app.status_message, "Reset view transform");

    // 5. :clear command
    app.show_report("Temporary Report".to_string());
    assert_eq!(app.modal.mode, merm_ui::UiMode::Report);
    app.execute_command_str(":clear");
    assert_eq!(app.modal.mode, merm_ui::UiMode::Normal);
    assert!(app.report_content.is_none());

    // 6. :q command
    assert!(app.is_running);
    app.execute_command_str(":q");
    assert!(!app.is_running);
}

#[test]
fn test_diagram_scaling_on_large_graph() {
    let mut transform = merm_render::Transform2D::default();

    // Large diagram (e.g. 126 classes in merm project: 4500x3200px)
    transform.fit_to_viewport(4500.0, 3200.0, 1280.0, 720.0);

    // Must be clamped to comfortable readable scale (>= 0.75) instead of microscopic 0.1x!
    assert!(
        transform.scale >= 0.75,
        "Scale on large graph must remain comfortable and legible, was {}",
        transform.scale
    );
    // Pan must align to top-left with padding so first modules are visible
    assert_eq!(transform.pan_x, 40.0);
    assert_eq!(transform.pan_y, 50.0);
}

#[test]
fn test_interactive_diagram_editing_commands_and_actions() {
    let source = "classDiagram\n    direction TD\n    class AuthService\n".to_string();
    let mut app = AppState::new(source, false);
    app.manifest = None;

    // 1. :add class OrderProcessor
    app.execute_command_str(":add class OrderProcessor");
    assert!(app.diagram_source.contains("class OrderProcessor"));
    assert!(app.report_content.as_ref().unwrap().contains("Added Node"));

    // 2. :connect AuthService OrderProcessor calls
    app.execute_command_str(":connect AuthService OrderProcessor calls");
    assert!(app
        .diagram_source
        .contains("AuthService --> OrderProcessor : calls"));
    assert!(app
        .report_content
        .as_ref()
        .unwrap()
        .contains("Node Connection"));

    // 3. :edit OrderProcessor
    app.execute_command_str(":edit OrderProcessor");
    assert_eq!(app.modal.mode, merm_ui::UiMode::NodeEdit);
    assert_eq!(app.modal.edit_node_id, "OrderProcessor");

    // 4. Save node edit
    app.handle_key_action(merm_ui::UiAction::SaveNodeEdit {
        node_id: "OrderProcessor".to_string(),
        stereotype: Some("service".to_string()),
        members: vec!["+process_order() -> bool".to_string()],
    });
    assert!(app.diagram_source.contains("class OrderProcessor {"));
    assert!(app.diagram_source.contains("<<service>>"));
    assert!(app.diagram_source.contains("+process_order() -> bool"));

    // 5. :rm OrderProcessor
    app.execute_command_str(":rm OrderProcessor");
    assert!(!app.diagram_source.contains("class OrderProcessor"));
    assert!(!app.diagram_source.contains("calls"));
    assert!(app.diagram_source.contains("class AuthService"));
}

#[test]
fn test_node_navigation_and_e_keybindings() {
    let mut ctrl = ModalController {
        active_test_node_id: Some("AuthService".to_string()),
        ..Default::default()
    };

    // 'e' and 'E' open node editor
    assert_eq!(
        ctrl.handle_key('e', true),
        UiAction::OpenNodeEditor("AuthService".to_string())
    );
    assert_eq!(
        ctrl.handle_key('E', true),
        UiAction::OpenNodeEditor("AuthService".to_string())
    );

    // Test AppState arrow key directional selection
    let source = "classDiagram\n    class NodeA\n    class NodeB\n    NodeA --> NodeB".to_string();
    let mut app = AppState::new(source, false);

    // If active node is None, any arrow navigation immediately selects first node (NodeA)
    app.select_node(None);
    assert!(app.active_node_id.is_none());
    app.select_directional_node(0.0, 1.0);
    assert_eq!(app.active_node_id.as_deref(), Some("NodeA"));

    // Next arrow press moves to next node (NodeB)
    app.select_directional_node(0.0, 1.0);
    assert_eq!(app.active_node_id.as_deref(), Some("NodeB"));

    // Canvas overlay SVG renders selection outline for active node
    let overlay = merm_ui::MermAppWindow::build_overlay_svg(&app, 1280, 720).unwrap();
    assert!(overlay.contains("🎯 SELECTED"));

    // 'e' opens node editor even if invoked without node_id argument
    app.execute_command_str(":edit");
    assert_eq!(app.modal.mode, merm_ui::UiMode::NodeEdit);
}
