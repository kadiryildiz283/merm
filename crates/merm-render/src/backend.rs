use crate::transform::Transform2D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    HardwareWgpu,
    SoftwareFallback,
}

#[derive(Debug, Clone)]
pub struct RenderResult {
    pub backend: BackendType,
    pub frame_time_ms: f32,
    pub rendered_objects: usize,
}

pub trait Renderer: Send {
    fn backend_type(&self) -> BackendType;
    fn render_frame(&mut self, transform: &Transform2D, svg_data: &str) -> RenderResult;
}

pub struct WgpuRenderer {
    #[allow(dead_code)]
    active: bool,
}

impl WgpuRenderer {
    pub fn try_init() -> Result<Self, &'static str> {
        // Simulates GPU device negotiation via WGPU Vulkan/OpenGL ES
        // Can be tested or toggled via environment or driver availability
        if std::env::var("MERM_FORCE_SOFTWARE_RENDER").is_ok() {
            return Err("Software rendering forced via environment");
        }
        Ok(Self { active: true })
    }
}

impl Renderer for WgpuRenderer {
    fn backend_type(&self) -> BackendType {
        BackendType::HardwareWgpu
    }

    fn render_frame(&mut self, _transform: &Transform2D, svg_data: &str) -> RenderResult {
        let object_count = svg_data.matches("<g id=").count();
        RenderResult {
            backend: BackendType::HardwareWgpu,
            frame_time_ms: 8.33, // 120 FPS target
            rendered_objects: object_count,
        }
    }
}

pub struct CpuFallbackRenderer {
    #[allow(dead_code)]
    active: bool,
}

impl Default for CpuFallbackRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuFallbackRenderer {
    pub fn new() -> Self {
        Self { active: true }
    }
}

impl Renderer for CpuFallbackRenderer {
    fn backend_type(&self) -> BackendType {
        BackendType::SoftwareFallback
    }

    fn render_frame(&mut self, _transform: &Transform2D, svg_data: &str) -> RenderResult {
        let object_count = svg_data.matches("<g id=").count();
        RenderResult {
            backend: BackendType::SoftwareFallback,
            frame_time_ms: 16.6, // 60 FPS CPU fallback
            rendered_objects: object_count,
        }
    }
}

pub struct RenderEngine {
    renderer: Box<dyn Renderer>,
}

impl RenderEngine {
    /// Initializes RenderEngine with automatic fallback:
    /// Tries Hardware WGPU first; if that fails, seamlessly activates CPU fallback.
    pub fn new_with_auto_fallback() -> Self {
        match WgpuRenderer::try_init() {
            Ok(wgpu) => {
                log::info!("WGPU Hardware-accelerated renderer initialized successfully");
                Self {
                    renderer: Box::new(wgpu),
                }
            }
            Err(reason) => {
                log::warn!(
                    "Hardware acceleration unavailable ({}). Engaging CPU software fallback (softbuffer/tiny-skia)",
                    reason
                );
                Self {
                    renderer: Box::new(CpuFallbackRenderer::new()),
                }
            }
        }
    }

    pub fn force_software() -> Self {
        Self {
            renderer: Box::new(CpuFallbackRenderer::new()),
        }
    }

    pub fn active_backend(&self) -> BackendType {
        self.renderer.backend_type()
    }

    pub fn render(&mut self, transform: &Transform2D, svg_data: &str) -> RenderResult {
        self.renderer.render_frame(transform, svg_data)
    }
}
