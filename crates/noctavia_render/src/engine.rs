use std::sync::Arc;
use wgpu::{
    Adapter, Device, Instance, Queue, Surface, SurfaceConfiguration, 
    TextureUsages, TextureView, Features, RequestAdapterOptions,
};
use winit::window::Window;
use futures::executor::block_on;

pub struct RenderEngine {
    pub instance: Instance,
    pub surface: Surface<'static>,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
}

impl RenderEngine {
    pub fn new(window: Arc<Window>) -> Self {
        let instance = Instance::default();
        let surface = instance.create_surface(window.clone()).expect("failed to create surface");
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })).expect("failed to find a suitable adapter");

        let mut required_features = Features::empty();
        if adapter.features().contains(Features::POLYGON_MODE_LINE) {
            required_features.insert(Features::POLYGON_MODE_LINE);
        }

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits: adapter.limits(),
                memory_hints: Default::default(),
                experimental_features: Default::default(),
                trace: wgpu::Trace::Off,
            },
        )).expect("failed to create a device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0]; // Simplified for now

        let size = window.inner_size();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: Vec::new(),
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn create_depth_texture(&self) -> TextureView {
        let size = wgpu::Extent3d {
            width: self.config.width,
            height: self.config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = self.device.create_texture(&desc);
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }
}
