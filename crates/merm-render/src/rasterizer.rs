use std::sync::Arc;
use crate::transform::Transform2D;

pub struct SvgRasterizer {
    fontdb: Arc<resvg::usvg::fontdb::Database>,
}

impl Default for SvgRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SvgRasterizer {
    pub fn new() -> Self {
        let mut fontdb = resvg::usvg::fontdb::Database::new();
        fontdb.load_system_fonts();
        Self {
            fontdb: Arc::new(fontdb),
        }
    }

    pub fn rasterize(
        &self,
        svg_data: &str,
        transform: &Transform2D,
        width: u32,
        height: u32,
        dest_buffer: &mut [u32],
    ) -> Result<(), String> {
        if width == 0 || height == 0 || dest_buffer.len() < (width * height) as usize {
            return Err("Invalid buffer dimensions".to_string());
        }

        let opt = resvg::usvg::Options {
            fontdb: self.fontdb.clone(),
            ..Default::default()
        };
        let tree = resvg::usvg::Tree::from_str(svg_data, &opt)
            .map_err(|e| format!("SVG parse error: {e}"))?;

        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| "Failed to allocate tiny-skia pixmap".to_string())?;

        // Catppuccin Mocha background: #1e1e2e (R:30, G:30, B:46)
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(30, 30, 46, 255));

        let render_ts = resvg::tiny_skia::Transform::from_scale(transform.scale, transform.scale)
            .post_translate(transform.pan_x, transform.pan_y);

        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        let rgba = pixmap.data();
        let (chunks, _) = rgba.as_chunks::<4>();
        for (i, chunk) in chunks.iter().enumerate() {
            if i < dest_buffer.len() {
                let r = chunk[0] as u32;
                let g = chunk[1] as u32;
                let b = chunk[2] as u32;
                dest_buffer[i] = (r << 16) | (g << 8) | b;
            }
        }

        Ok(())
    }
}
