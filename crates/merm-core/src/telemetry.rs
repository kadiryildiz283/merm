use std::time::Instant;

#[derive(Debug, Clone)]
pub struct PerformanceTelemetry {
    pub enabled: bool,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub input_latency_ms: f32,
    pub update_time_ms: f32,
    pub render_time_ms: f32,
    pub visible_nodes: usize,
    pub total_nodes: usize,
    pub cache_hit_rate: f32,
    pub cache_hits: u64,
    pub cache_misses: u64,
    last_frame_instant: Option<Instant>,
    frame_durations: Vec<f32>,
}

impl Default for PerformanceTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceTelemetry {
    pub fn new() -> Self {
        Self {
            enabled: false,
            fps: 120.0,
            frame_time_ms: 8.33,
            input_latency_ms: 0.3,
            update_time_ms: 0.8,
            render_time_ms: 4.5,
            visible_nodes: 0,
            total_nodes: 0,
            cache_hit_rate: 100.0,
            cache_hits: 0,
            cache_misses: 0,
            last_frame_instant: None,
            frame_durations: Vec::with_capacity(60),
        }
    }

    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    pub fn record_cache_hit(&mut self) {
        self.cache_hits = self.cache_hits.saturating_add(1);
        self.update_cache_rate();
    }

    pub fn record_cache_miss(&mut self) {
        self.cache_misses = self.cache_misses.saturating_add(1);
        self.update_cache_rate();
    }

    fn update_cache_rate(&mut self) {
        let total = self.cache_hits + self.cache_misses;
        if total > 0 {
            self.cache_hit_rate = (self.cache_hits as f32 / total as f32) * 100.0;
        }
    }

    pub fn begin_frame(&mut self) -> Instant {
        Instant::now()
    }

    pub fn record_frame(
        &mut self,
        frame_start: Instant,
        update_ms: f32,
        render_ms: f32,
        visible_nodes: usize,
        total_nodes: usize,
    ) {
        let elapsed = frame_start.elapsed().as_secs_f32() * 1000.0;
        self.frame_time_ms = elapsed;
        self.update_time_ms = update_ms;
        self.render_time_ms = render_ms;
        self.visible_nodes = visible_nodes;
        self.total_nodes = total_nodes;

        // Moving average for FPS calculation over last 60 frames
        if let Some(prev) = self.last_frame_instant {
            let delta = prev.elapsed().as_secs_f32();
            if delta > 0.0001 {
                let current_fps = 1.0 / delta;
                if self.frame_durations.len() >= 60 {
                    self.frame_durations.remove(0);
                }
                self.frame_durations.push(current_fps);
                let sum: f32 = self.frame_durations.iter().sum();
                self.fps = (sum / self.frame_durations.len() as f32).clamp(1.0, 360.0);
            }
        }
        self.last_frame_instant = Some(Instant::now());
    }

    pub fn summary_string(&self) -> String {
        format!(
            "FPS: {:>5.1} │ FRAME: {:>4.1}ms │ UPD: {:>4.1}ms │ RND: {:>4.1}ms │ NODES: {}/{} │ CACHE: {:>4.1}%",
            self.fps,
            self.frame_time_ms,
            self.update_time_ms,
            self.render_time_ms,
            self.visible_nodes,
            self.total_nodes,
            self.cache_hit_rate
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_metrics_tracking() {
        let mut telem = PerformanceTelemetry::new();
        assert!(!telem.enabled);
        assert!(telem.toggle());
        assert!(telem.enabled);

        telem.record_cache_hit();
        telem.record_cache_hit();
        telem.record_cache_miss();
        assert!((telem.cache_hit_rate - 66.66).abs() < 1.0);

        let t0 = telem.begin_frame();
        telem.record_frame(t0, 0.5, 3.2, 12, 12);
        assert_eq!(telem.visible_nodes, 12);
        assert_eq!(telem.total_nodes, 12);
        assert!(telem.render_time_ms >= 3.0);
    }
}
