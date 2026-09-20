use merm_core::{AstRewriter, DiagramExtractor, DiagramType, LayoutDirection, LayoutEngine};

#[test]
fn test_markdown_extractor() {
    let markdown = r#"
# Architecture

Here is the context:

```mermaid
flowchart TD
    A[Client] --> B[Server]
    B --> C[Database]
```

Some documentation text.
"#;

    let blocks = DiagramExtractor::extract(markdown).expect("Must extract blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].diagram_type, DiagramType::Flowchart);
    assert!(blocks[0].source.contains("A[Client]"));
}

#[test]
fn test_ast_direction_pivoting() {
    let source = "flowchart TD\n    A --> B";
    let pivoted = AstRewriter::pivot_direction(source, LayoutDirection::LR);
    assert!(pivoted.contains("flowchart LR"));
}

#[test]
fn test_ast_node_isolation() {
    let source =
        "flowchart TD\n    A[Order] --> B[Billing]\n    B --> C[Shipping]\n    D[Auth] --> A";
    let isolated = AstRewriter::isolate_node(source, "Billing");
    assert!(isolated.contains("Billing"));
    assert!(!isolated.contains("Shipping"));
}

#[test]
fn test_layout_engine_rendering() {
    let engine = LayoutEngine::default();
    let source = "flowchart TD\n    A[Frontend] --> B[Backend]";
    let rendered = engine.render_with_watchdog(source).expect("Must render");
    assert!(rendered.svg.contains("<svg"));
    assert!(rendered.svg.contains("Frontend"));
    assert_eq!(rendered.nodes.len(), 2);
}
