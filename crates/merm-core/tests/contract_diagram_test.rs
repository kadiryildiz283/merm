use merm_core::{ClassDiagramParser, ColorPalette, LayoutDirection};

#[test]
fn test_contract_compartment_surfaced_in_class_diagram() {
    let source = r#"classDiagram
    direction TD
    class OrderService {
        <<struct>>
        +order_id: u64
        +process() bool
    }
"#;

    let palette = ColorPalette::default();
    let diag =
        ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::TD), Some(&palette))
            .expect("Failed to parse and render class diagram");

    assert_eq!(diag.nodes.len(), 1);
    let node = &diag.nodes[0];
    assert!(node.contract.is_some(), "Node must have contract info");

    let contract = node.contract.as_ref().unwrap();
    assert_eq!(contract.input_default.as_deref(), Some("{}"));
    assert_eq!(contract.output_default.as_deref(), Some("1"));

    // Check that SVG contains visible CONTRACT compartment
    assert!(diag.svg.contains("IN </tspan>"));
    assert!(diag.svg.contains("OUT</tspan>"));
    assert!(diag.svg.contains("Def: {}"));
    assert!(
        diag.svg.contains("Def: &quot;1&quot;")
            || diag.svg.contains("Def: 1")
            || diag.svg.contains("Def: &#34;1&#34;")
    );
}

#[test]
fn test_active_edge_emphasis_when_node_selected() {
    let source = r#"classDiagram
    class Alpha
    class Beta
    class Gamma

    Alpha --> Beta : calls
    Beta --> Gamma : routes
"#;

    let palette = ColorPalette::default();
    let mut diag =
        ClassDiagramParser::parse_and_render(source, Some(LayoutDirection::TD), Some(&palette))
            .expect("Failed to parse diagram");

    // Select Alpha
    diag.selected_node_id = Some("Alpha".to_string());
    diag.regenerate_svg(&palette);

    // Connected edge (Alpha --> Beta) should have full opacity, while unrelated (Beta --> Gamma) is dimmed
    assert!(diag.svg.contains("opacity=\"1.0\""));
    assert!(diag.svg.contains("opacity=\"0.35\""));
}
