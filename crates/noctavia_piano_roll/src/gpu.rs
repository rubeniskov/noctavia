use std::sync::Arc;

use futures::executor::block_on;
use wgpu::{
    Device,
    DeviceDescriptor,
    Features,
    Instance,
    PresentMode,
    Queue,
    RequestAdapterOptions,
    Surface,
    SurfaceConfiguration,
    TextureUsages,
};
use winit::window::Window;

pub struct GpuContext<'w> {
    pub instance: Instance,
    pub surface: Surface<'w>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub supports_wireframe: bool,
}

impl<'w> GpuContext<'w> {
    pub fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = Instance::default();
        let surface = instance
            .create_surface(window)
            .expect("failed to create surface");

        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .expect("failed to find a suitable adapter");

        let mut features = Features::empty();
        let supports_wireframe = adapter.features().contains(Features::POLYGON_MODE_LINE);
        if supports_wireframe {
            features.insert(Features::POLYGON_MODE_LINE);
        }

        let (device, queue) = block_on(adapter.request_device(
            &DeviceDescriptor {
                label: Some("device"),
                required_features: features,
                required_limits: adapter.limits(),
            },
            None,
        ))
        .expect("failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            instance,
            surface,
            device,
            queue,
            config,
            supports_wireframe,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}