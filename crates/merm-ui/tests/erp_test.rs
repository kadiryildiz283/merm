use merm_core::{DiagramExtractor, LayoutEngine};
use merm_render::{SvgRasterizer, Transform2D};
use std::fs;
use std::path::Path;

#[test]
fn test_rasterize_erp_mermaid() {
    let local_path = "/home/kadir/Projeler/erp/mermaid.md";
    let content = if Path::new(local_path).exists() {
        fs::read_to_string(local_path).expect("File must exist")
    } else {
        r#"```mermaid
flowchart TD
    A[Start Node] --> B[Process Task]
    B -->|"(1 Kez) Tekrarlı Bildirim"| C[Notification Hub]
    C -->|"(Red Alert) Kırmızı Alarm"| D[Alert Dispatcher]
```"#
            .to_string()
    };
    let extracted = DiagramExtractor::extract(&content).expect("Must extract blocks");
    let engine = LayoutEngine::default();
    let diagram = engine
        .render_with_watchdog(&extracted[0].source)
        .expect("Must render diagram");

    // Ensure edge label text with parentheses is not treated as phantom nodes
    assert!(
        !diagram.nodes.iter().any(|n| n.id.contains("1 Kez")),
        "Phantom node '1 Kez' should not exist"
    );
    assert!(
        !diagram.nodes.iter().any(|n| n.id.contains("Red Alert")),
        "Phantom node 'Red Alert' should not exist"
    );

    let rasterizer = SvgRasterizer::new();
    let mut transform = Transform2D::default();
    transform.fit_to_viewport(diagram.width, diagram.height, 1280.0, 720.0);

    let mut buffer = vec![0u32; 1280 * 720];
    let res = rasterizer.rasterize(
        &diagram.svg,
        &transform,
        1280,
        720,
        &mut buffer,
        Some(0x1e1e2e),
    );

    if let Err(ref e) = res {
        eprintln!("RASTERIZE ERROR: {}", e);
    }
    assert!(res.is_ok(), "Rasterize failed: {:?}", res);
}
