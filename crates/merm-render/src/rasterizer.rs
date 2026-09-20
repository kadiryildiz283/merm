use std::sync::{Arc, Mutex};
use crate::transform::Transform2D;

pub struct SvgRasterizer {
    fontdb: Arc<resvg::usvg::fontdb::Database>,
    cached_tree: Mutex<Option<(String, Arc<resvg::usvg::Tree>)>>,
    cached_pixmap: Mutex<Option<resvg::tiny_skia::Pixmap>>,
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
            cached_tree: Mutex::new(None),
            cached_pixmap: Mutex::new(None),
        }
    }

    pub fn rasterize(
        &self,
        svg_data: &str,
        transform: &Transform2D,
        width: u32,
        height: u32,
        dest_buffer: &mut [u32],
        bg_color: Option<u32>,
    ) -> Result<(), String> {
        if width == 0 || height == 0 || dest_buffer.len() < (width * height) as usize {
            return Err("Invalid buffer dimensions".to_string());
        }

        let tree = {
            let mut cache = self.cached_tree.lock().unwrap();
            if let Some((ref cached_str, ref tree)) = *cache {
                if cached_str == svg_data {
                    tree.clone()
                } else {
                    let opt = resvg::usvg::Options {
                        fontdb: self.fontdb.clone(),
                        ..Default::default()
                    };
                    let new_tree = Arc::new(
                        resvg::usvg::Tree::from_str(svg_data, &opt)
                            .map_err(|e| format!("SVG parse error: {e}"))?,
                    );
                    *cache = Some((svg_data.to_string(), new_tree.clone()));
                    new_tree
                }
            } else {
                let opt = resvg::usvg::Options {
                    fontdb: self.fontdb.clone(),
                    ..Default::default()
                };
                let new_tree = Arc::new(
                    resvg::usvg::Tree::from_str(svg_data, &opt)
                        .map_err(|e| format!("SVG parse error: {e}"))?,
                );
                *cache = Some((svg_data.to_string(), new_tree.clone()));
                new_tree
            }
        };

        let (bg_r, bg_g, bg_b) = if let Some(c) = bg_color {
            (((c >> 16) & 0xFF) as u8, ((c >> 8) & 0xFF) as u8, (c & 0xFF) as u8)
        } else {
            (30, 30, 46)
        };

        let mut pixmap_guard = self.cached_pixmap.lock().unwrap();
        let mut pixmap = match pixmap_guard.take() {
            Some(mut p) if p.width() == width && p.height() == height => {
                p.fill(resvg::tiny_skia::Color::from_rgba8(bg_r, bg_g, bg_b, 255));
                p
            }
            _ => {
                let mut p = resvg::tiny_skia::Pixmap::new(width, height)
                    .ok_or_else(|| "Failed to allocate tiny-skia pixmap".to_string())?;
                p.fill(resvg::tiny_skia::Color::from_rgba8(bg_r, bg_g, bg_b, 255));
                p
            }
        };

        let render_ts = resvg::tiny_skia::Transform::from_scale(transform.scale, transform.scale)
            .post_translate(transform.pan_x, transform.pan_y);

        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        let rgba = pixmap.data();
        let (chunks, _) = rgba.as_chunks::<4>();
        let len = chunks.len().min(dest_buffer.len());
        for (dst, chunk) in dest_buffer[..len].iter_mut().zip(&chunks[..len]) {
            *dst = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        }

        *pixmap_guard = Some(pixmap);
        Ok(())
    }
}
