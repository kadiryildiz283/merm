use merm_render::{SvgRasterizer, Transform2D};

#[test]
fn test_svg_rasterizer_renders_pixels() {
    let rasterizer = SvgRasterizer::new();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="10" y="10" width="80" height="80" fill="#89b4fa"/></svg>"##;
    let transform = Transform2D::default();
    let mut buffer = vec![0u32; 100 * 100];

    let res = rasterizer.rasterize(svg, &transform, 100, 100, &mut buffer, None);
    assert!(res.is_ok());

    // Verify some pixels are not black (background is 0x1e1e2e, rect is 0x89b4fa)
    let non_zero_count = buffer.iter().filter(|&&px| px != 0).count();
    assert_eq!(non_zero_count, 10000);
    // Center pixel (50, 50) should be #89b4fa (r: 137, g: 180, b: 250 -> 0x89b4fa)
    let center_px = buffer[50 * 100 + 50];
    assert_eq!(center_px, 0x89b4fa);
}
