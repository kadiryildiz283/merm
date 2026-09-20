#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub pan_x: f32,
    pub pan_y: f32,
    pub scale: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            scale: 1.0,
        }
    }
}

impl Transform2D {
    pub fn new(pan_x: f32, pan_y: f32, scale: f32) -> Self {
        Self { pan_x, pan_y, scale }
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    pub fn zoom_at(&mut self, factor: f32, center_x: f32, center_y: f32) {
        let old_scale = self.scale;
        let new_scale = (old_scale * factor).clamp(0.05, 20.0);

        // Keep the cursor focal point stationary on screen during zoom
        self.pan_x = center_x - (center_x - self.pan_x) * (new_scale / old_scale);
        self.pan_y = center_y - (center_y - self.pan_y) * (new_scale / old_scale);
        self.scale = new_scale;
    }

    pub fn screen_to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        let wx = (sx - self.pan_x) / self.scale;
        let wy = (sy - self.pan_y) / self.scale;
        (wx, wy)
    }

    pub fn world_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        let sx = wx * self.scale + self.pan_x;
        let sy = wy * self.scale + self.pan_y;
        (sx, sy)
    }

    pub fn fit_to_viewport(&mut self, content_w: f32, content_h: f32, view_w: f32, view_h: f32) {
        if content_w <= 0.0 || content_h <= 0.0 || view_w <= 0.0 || view_h <= 0.0 {
            return;
        }

        let padding = 40.0;
        let avail_w = (view_w - padding * 2.0).max(1.0);
        let avail_h = (view_h - padding * 2.0).max(1.0);

        let scale_x = avail_w / content_w;
        let scale_y = avail_h / content_h;
        self.scale = scale_x.min(scale_y).clamp(0.1, 5.0);

        self.pan_x = (view_w - content_w * self.scale) / 2.0;
        self.pan_y = (view_h - content_h * self.scale) / 2.0;
    }
}
