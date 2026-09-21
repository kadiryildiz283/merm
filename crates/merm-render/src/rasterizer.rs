use crate::transform::Transform2D;
use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct CachedDiagramRender {
    svg_hash: u64,
    scale: f32,
    pan_x: f32,
    pan_y: f32,
    width: u32,
    height: u32,
    bg_color: Option<u32>,
    pixels: Vec<u32>,
}

pub struct SvgRasterizer {
    fontdb: Arc<resvg::usvg::fontdb::Database>,
    cached_tree: Mutex<Option<(String, Arc<resvg::usvg::Tree>)>>,
    cached_pixmap: Mutex<Option<resvg::tiny_skia::Pixmap>>,
    cached_diagram_render: Mutex<Option<CachedDiagramRender>>,
    cached_overlay_render: Mutex<Option<(u64, u32, u32, resvg::tiny_skia::Pixmap)>>,
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
            if fontdb
                .faces()
                .any(|f| f.families.iter().any(|(name, _)| name == *cand))
            {
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
            if fontdb
                .faces()
                .any(|f| f.families.iter().any(|(name, _)| name == *cand))
            {
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
            cached_diagram_render: Mutex::new(None),
            cached_overlay_render: Mutex::new(None),
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
    ) -> Result<bool, String> {
        if width == 0 || height == 0 || dest_buffer.len() < (width * height) as usize {
            return Err("Invalid buffer dimensions".to_string());
        }

        // Fast path: Check diagram raster cache (avoids expensive SVG re-parsing and re-rendering)
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        svg_data.hash(&mut hasher);
        let svg_hash = hasher.finish();

        {
            let cache_guard = self.cached_diagram_render.lock().unwrap();
            if let Some(ref c) = *cache_guard {
                if c.svg_hash == svg_hash
                    && (c.scale - transform.scale).abs() < 1e-4
                    && (c.pan_x - transform.pan_x).abs() < 1e-4
                    && (c.pan_y - transform.pan_y).abs() < 1e-4
                    && c.width == width
                    && c.height == height
                    && c.bg_color == bg_color
                    && dest_buffer.len() >= c.pixels.len()
                {
                    dest_buffer[..c.pixels.len()].copy_from_slice(&c.pixels);
                    return Ok(true);
                }
            }
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

        let bg_fill_color = match bg_color {
            Some(0) => resvg::tiny_skia::Color::TRANSPARENT,
            Some(c) => {
                let a = ((c >> 24) & 0xFF) as u8;
                let r = ((c >> 16) & 0xFF) as u8;
                let g = ((c >> 8) & 0xFF) as u8;
                let b = (c & 0xFF) as u8;
                let alpha = if a == 0 && (r != 0 || g != 0 || b != 0) {
                    255
                } else {
                    a
                };
                resvg::tiny_skia::Color::from_rgba8(r, g, b, alpha)
            }
            None => resvg::tiny_skia::Color::from_rgba8(30, 30, 46, 255),
        };

        let mut pixmap_guard = self.cached_pixmap.lock().unwrap();
        let mut pixmap = match pixmap_guard.take() {
            Some(mut p) if p.width() == width && p.height() == height => {
                p.fill(bg_fill_color);
                p
            }
            _ => {
                let mut p = resvg::tiny_skia::Pixmap::new(width, height)
                    .ok_or_else(|| "Failed to allocate tiny-skia pixmap".to_string())?;
                p.fill(bg_fill_color);
                p
            }
        };

        let render_ts = resvg::tiny_skia::Transform::from_scale(transform.scale, transform.scale)
            .post_translate(transform.pan_x, transform.pan_y);

        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        let rgba = pixmap.data();
        let (chunks, _) = rgba.as_chunks::<4>();
        let len = chunks.len().min(dest_buffer.len());
        dest_buffer[..len]
            .par_chunks_mut(16384)
            .zip(chunks[..len].par_chunks(16384))
            .for_each(|(dst_slice, src_slice)| {
                for (dst, chunk) in dst_slice.iter_mut().zip(src_slice.iter()) {
                    *dst = ((chunk[3] as u32) << 24)
                        | ((chunk[0] as u32) << 16)
                        | ((chunk[1] as u32) << 8)
                        | (chunk[2] as u32);
                }
            });

        // Cache rendered pixels for instantaneous redraw during typing / split scrolling
        {
            let mut cache_guard = self.cached_diagram_render.lock().unwrap();
            *cache_guard = Some(CachedDiagramRender {
                svg_hash,
                scale: transform.scale,
                pan_x: transform.pan_x,
                pan_y: transform.pan_y,
                width,
                height,
                bg_color,
                pixels: dest_buffer[..len].to_vec(),
            });
        }

        *pixmap_guard = Some(pixmap);
        Ok(false)
    }

    pub fn invalidate_cache(&self) {
        if let Ok(mut g) = self.cached_diagram_render.lock() {
            *g = None;
        }
        if let Ok(mut g) = self.cached_overlay_render.lock() {
            *g = None;
        }
    }

    pub fn rasterize_overlay(
        &self,
        svg_data: &str,
        width: u32,
        height: u32,
        dest_buffer: &mut [u32],
    ) -> Result<(), String> {
        if width == 0 || height == 0 || dest_buffer.len() < (width * height) as usize {
            return Err("Invalid buffer dimensions".to_string());
        }

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        svg_data.hash(&mut hasher);
        let overlay_hash = hasher.finish();

        // 1. Check if cached overlay pixmap matches
        let cached_hit = {
            let guard = self.cached_overlay_render.lock().unwrap();
            if let Some((h, w, hgt, ref p)) = *guard {
                if h == overlay_hash && w == width && hgt == height {
                    Some(p.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let pixmap = if let Some(p) = cached_hit {
            p
        } else {
            let opt = resvg::usvg::Options {
                fontdb: self.fontdb.clone(),
                ..Default::default()
            };
            let tree = resvg::usvg::Tree::from_str(svg_data, &opt)
                .map_err(|e| format!("Failed to parse overlay SVG: {}", e))?;

            let mut p = resvg::tiny_skia::Pixmap::new(width, height)
                .ok_or_else(|| "Failed to allocate tiny-skia pixmap".to_string())?;

            let render_ts = resvg::tiny_skia::Transform::identity();
            resvg::render(&tree, render_ts, &mut p.as_mut());

            let mut guard = self.cached_overlay_render.lock().unwrap();
            *guard = Some((overlay_hash, width, height, p.clone()));
            p
        };

        // 2. High-performance Rayon multi-threaded alpha-blend
        let rgba = pixmap.data();
        let (chunks, _) = rgba.as_chunks::<4>();
        let len = chunks.len().min(dest_buffer.len());

        dest_buffer[..len]
            .par_chunks_mut(16384)
            .zip(chunks[..len].par_chunks(16384))
            .for_each(|(dst_slice, src_slice)| {
                for (dst, chunk) in dst_slice.iter_mut().zip(src_slice.iter()) {
                    let src_a = chunk[3] as u32;
                    if src_a == 255 {
                        *dst = 0xff00_0000
                            | ((chunk[0] as u32) << 16)
                            | ((chunk[1] as u32) << 8)
                            | (chunk[2] as u32);
                    } else if src_a > 0 {
                        let inv_a = 255 - src_a;
                        let dst_val = *dst;
                        let dst_r = (dst_val >> 16) & 0xff;
                        let dst_g = (dst_val >> 8) & 0xff;
                        let dst_b = dst_val & 0xff;
                        let out_r = ((chunk[0] as u32 * src_a + dst_r * inv_a) / 255) & 0xff;
                        let out_g = ((chunk[1] as u32 * src_a + dst_g * inv_a) / 255) & 0xff;
                        let out_b = ((chunk[2] as u32 * src_a + dst_b * inv_a) / 255) & 0xff;
                        *dst = 0xff00_0000 | (out_r << 16) | (out_g << 8) | out_b;
                    }
                }
            });

        Ok(())
    }

    pub fn rasterize_to_png(
        &self,
        svg_data: &str,
        scale: f32,
        bg_color: Option<u32>,
    ) -> Result<Vec<u8>, String> {
        let opt = resvg::usvg::Options {
            fontdb: self.fontdb.clone(),
            ..Default::default()
        };
        let tree = resvg::usvg::Tree::from_str(svg_data, &opt)
            .map_err(|e| format!("SVG parse error: {e}"))?;

        let size = tree.size();
        let width = ((size.width() * scale).ceil() as u32).max(1);
        let height = ((size.height() * scale).ceil() as u32).max(1);

        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
            .ok_or_else(|| "Failed to allocate tiny-skia pixmap".to_string())?;

        let bg = match bg_color {
            Some(0) => resvg::tiny_skia::Color::TRANSPARENT,
            Some(c) => {
                let a = ((c >> 24) & 0xFF) as u8;
                let r = ((c >> 16) & 0xFF) as u8;
                let g = ((c >> 8) & 0xFF) as u8;
                let b = (c & 0xFF) as u8;
                resvg::tiny_skia::Color::from_rgba8(
                    r,
                    g,
                    b,
                    if a == 0 && (r != 0 || g != 0 || b != 0) {
                        255
                    } else {
                        a
                    },
                )
            }
            None => resvg::tiny_skia::Color::from_rgba8(30, 30, 46, 255),
        };
        pixmap.fill(bg);

        let render_ts = resvg::tiny_skia::Transform::from_scale(scale, scale);
        resvg::render(&tree, render_ts, &mut pixmap.as_mut());

        pixmap
            .encode_png()
            .map_err(|e| format!("PNG encode error: {e}"))
    }
}
