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

        // Configure generic font family fallbacks to eliminate 'No match for monospace' warnings
        let mono_candidates = [
            "Noto Sans Mono",
            "DejaVu Sans Mono",
            "Adwaita Mono",
            "Fira Code",
            "Bitstream Vera Sans Mono",
            "Liberation Mono",
            "Consolas",
            "Monospace",
        ];
        let mut mono_match = None;
        for cand in &mono_candidates {
            if fontdb.faces().any(|f| f.families.iter().any(|(name, _)| name == *cand)) {
                mono_match = Some((*cand).to_string());
                break;
            }
        }
        let fallback_face_family = fontdb
            .faces()
            .next()
            .and_then(|f| f.families.first().map(|(name, _)| name.clone()));

        if let Some(m) = mono_match.or_else(|| fallback_face_family.clone()) {
            fontdb.set_monospace_family(m);
        }

        let sans_candidates = [
            "Noto Sans",
            "DejaVu Sans",
            "Adwaita Sans",
            "Fira Sans",
            "Liberation Sans",
            "Sans",
        ];
        let mut sans_match = None;
        for cand in &sans_candidates {
            if fontdb.faces().any(|f| f.families.iter().any(|(name, _)| name == *cand)) {
                sans_match = Some((*cand).to_string());
                break;
            }
        }
        if let Some(s) = sans_match.or(fallback_face_family) {
            fontdb.set_sans_serif_family(s);
        }

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

        // Fill background with canvas color or transparent if SVG specifies rect
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
