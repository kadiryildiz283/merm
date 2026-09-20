use std::fs;
use merm_core::{DiagramExtractor, LayoutEngine};
use merm_render::{SvgRasterizer, Transform2D};

#[test]
fn test_rasterize_erp_mermaid() {
    let path = "/home/kadir/Projeler/erp/mermaid.md";
    let content = fs::read_to_string(path).expect("File must exist");
    let extracted = DiagramExtractor::extract(&content).expect("Must extract blocks");
    let engine = LayoutEngine::default();
    let diagram = engine.render_with_watchdog(&extracted[0].source).expect("Must render diagram");

    let rasterizer = SvgRasterizer::new();
    let mut transform = Transform2D::default();
    transform.fit_to_viewport(diagram.width, diagram.height, 1280.0, 720.0);

    let mut buffer = vec![0u32; 1280 * 720];
    let res = rasterizer.rasterize(&diagram.svg, &transform, 1280, 720, &mut buffer, Some(0x1e1e2e));

    if let Err(ref e) = res {
        eprintln!("RASTERIZE ERROR: {}", e);
    }
    assert!(res.is_ok(), "Rasterize failed: {:?}", res);
}
