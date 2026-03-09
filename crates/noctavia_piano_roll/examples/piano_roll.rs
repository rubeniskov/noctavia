use std::{borrow::Cow, iter::once, mem::size_of, sync::Arc, time::Instant};
use bytemuck_derive::{Pod, Zeroable};
use futures::executor::block_on;
use wgpu::{
    util::DeviceExt, BufferUsages, Color, CommandEncoderDescriptor, DeviceDescriptor, Features,
    FragmentState, IndexFormat, Instance, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PresentMode, PrimitiveState, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor,
    ShaderSource, SurfaceConfiguration, TextureUsages, TextureViewDescriptor,
    VertexState,
};
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowAttributes,
};
use nalgebra_glm as glm;

use noctavia_piano_roll::instance::KeyInstance;
use noctavia_render::{Vertex, GpuMesh};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct Globals {
    view_proj: [f32; 16],
    light_view_proj: [f32; 16],
    camera_pos: [f32; 4],
}

fn main() {
    let event_loop = EventLoop::new().unwrap();

    let window = Arc::new(
        #[allow(deprecated)]
        event_loop
            .create_window(
                WindowAttributes::default()
                    .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
                    .with_title("Noctavia Piano Roll - Realistic 3D"),
            )
            .expect("failed to create a window"),
    );

    let mut _physical_size = window.inner_size();

    let instance = Instance::default();
    let surface = instance.create_surface(window.clone()).expect("failed to create surface");
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .expect("failed to find a suitable adapter");

    let (device, queue) = block_on(adapter.request_device(
        &DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            required_limits: adapter.limits(),
            memory_hints: Default::default(),
            experimental_features: Default::default(),
            trace: wgpu::Trace::Off,
        },
    ))
    .expect("failed to create a device");

    let shader_src = include_str!("shader.wgsl");
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
    });

    // --- BIND GROUP LAYOUTS ---
    let global_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("global_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let shadow_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&global_bind_group_layout, &shadow_bind_group_layout],
        push_constant_ranges: &[],
    });

    let shadow_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&global_bind_group_layout],
        push_constant_ranges: &[],
    });

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let swapchain_format = swapchain_capabilities
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(swapchain_capabilities.formats[0]);

    // --- PIPELINES ---
    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("piano_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Vertex::layout(), KeyInstance::layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(swapchain_format.into())],
            compilation_options: Default::default(),
        }),
        primitive: PrimitiveState {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Front),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let shadow_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("shadow_pipeline"),
        layout: Some(&shadow_pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_shadow"),
            buffers: &[Vertex::layout(), KeyInstance::layout()],
            compilation_options: Default::default(),
        },
        fragment: None,
        primitive: PrimitiveState {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Front),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // --- MESHES ---
    // Total notch width should accommodate black key (0.58).
    // Let's use 0.35 + 0.35 = 0.70 total gap.
    let white_plain = GpuMesh::from_mesh(&device, &noctavia_piano_roll::mesh::generate_white_key(0.0, 0.0));
    let white_cf = GpuMesh::from_mesh(&device, &noctavia_piano_roll::mesh::generate_white_key(0.0, 0.35));
    let white_dga = GpuMesh::from_mesh(&device, &noctavia_piano_roll::mesh::generate_white_key(0.35, 0.35));
    let white_eb = GpuMesh::from_mesh(&device, &noctavia_piano_roll::mesh::generate_white_key(0.35, 0.0));
    let black_mesh = GpuMesh::from_mesh(&device, &noctavia_piano_roll::mesh::generate_black_key());

    // --- UNIFORMS & BUFFERS ---
    let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("globals_buffer"),
        size: size_of::<Globals>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("global_bind_group"),
        layout: &global_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buffer.as_entire_binding(),
        }],
    });

    // --- SHADOW MAP ASSETS ---
    let shadow_size = 4096u32;
    let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shadow_texture"),
        size: wgpu::Extent3d {
            width: shadow_size,
            height: shadow_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("shadow_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });

    let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_bind_group"),
        layout: &shadow_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&shadow_sampler),
            },
        ],
    });

    let mut config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: 1280,
        height: 720,
        present_mode: PresentMode::Fifo,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: Vec::new(),
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    let mut depth_texture = create_depth_texture(&device, &config);

    // Keyboard state
    let mut key_target = [0.0f32; 128];
    let mut key_current = [0.0f32; 128];
    let mut last_frame = Instant::now();
    
    #[allow(deprecated)]
    event_loop
        .run(move |event, target| {
            target.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        target.exit();
                    }

                    WindowEvent::Resized(size) => {
                        if size.width > 0 && size.height > 0 {
                            _physical_size = size;
                            config.width = size.width;
                            config.height = size.height;
                            surface.configure(&device, &config);
                            depth_texture = create_depth_texture(&device, &config);
                        }
                    }

                    WindowEvent::KeyboardInput { event, .. } => {
                        if let PhysicalKey::Code(key_code) = event.physical_key {
                            let intensity = if event.state == ElementState::Pressed { 1.0 } else { 0.0 };
                            match key_code {
                                KeyCode::KeyA => key_target[60] = intensity,
                                KeyCode::KeyW => key_target[61] = intensity,
                                KeyCode::KeyS => key_target[62] = intensity,
                                KeyCode::KeyE => key_target[63] = intensity,
                                KeyCode::KeyD => key_target[64] = intensity,
                                KeyCode::KeyF => key_target[65] = intensity,
                                KeyCode::KeyT => key_target[66] = intensity,
                                KeyCode::KeyG => key_target[67] = intensity,
                                KeyCode::KeyY => key_target[68] = intensity,
                                KeyCode::KeyH => key_target[69] = intensity,
                                KeyCode::KeyU => key_target[70] = intensity,
                                KeyCode::KeyJ => key_target[71] = intensity,
                                KeyCode::KeyK => key_target[72] = intensity,
                                _ => (),
                            }
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt = now.duration_since(last_frame).as_secs_f32();
                        last_frame = now;

                        for i in 0..128 {
                            let t = key_target[i];
                            let c = key_current[i];
                            if t > c {
                                key_current[i] = (c + dt * 25.0).min(t);
                            } else {
                                key_current[i] = (c - dt * 10.0).max(t);
                            }
                        }

                        // --- CAMERA MATRICES ---
                        let aspect = config.width as f32 / config.height as f32;
                        let piano_width = 52.0;
                        let view_width = piano_width;
                        let view_height = view_width / aspect;
                        
                        let mut projection = glm::ortho_lh(
                            -view_width / 2.0, view_width / 2.0, 
                            -view_height / 2.0, view_height / 2.0, 
                            0.1, 100.0
                        );
                        let correction = glm::mat4(
                            1.0, 0.0, 0.0, 0.0,
                            0.0, 1.0, 0.0, 0.0,
                            0.0, 0.0, 0.5, 0.5,
                            0.0, 0.0, 0.0, 1.0,
                        );
                        projection = correction * projection;
                        
                        let mut view = glm::Mat4::identity();
                        view = glm::translate(&view, &glm::vec3(0.0, 0.0, 10.0));
                        view = glm::rotate(&view, 0.75, &glm::vec3(1.0, 0.0, 0.0));
                        view = glm::translate(&view, &glm::vec3(-piano_width / 2.0, -view_height / 2.0 + 4.8, 0.0));
                        
                        // --- LIGHT CAMERA (LookAt for stability) ---
                        let light_pos = glm::vec3(-10.0, -15.0, 25.0);
                        let light_target = glm::vec3(0.0, 0.0, 0.0);
                        let light_up = glm::vec3(0.0, 0.0, 1.0);
                        let mut light_view = glm::look_at_lh(&light_pos, &light_target, &light_up);
                        // Shift light camera to cover the full keyboard width
                        light_view = glm::translate(&light_view, &glm::vec3(-piano_width / 2.0, 0.0, 0.0));
                        
                        let light_proj = glm::ortho_lh(-35.0, 35.0, -35.0, 35.0, 0.1, 100.0);
                        let light_view_proj = correction * light_proj * light_view;
                        
                        let mut vp_array = [0.0f32; 16];
                        vp_array.copy_from_slice(glm::value_ptr(&(projection * view)));
                        let mut lvp_array = [0.0f32; 16];
                        lvp_array.copy_from_slice(glm::value_ptr(&light_view_proj));
                        
                        // Extract camera position from the view matrix
                        // view = rotate * translate, so inverse view gives camera position in column 3
                        let inv_view = glm::inverse(&view);
                        let cam_pos = glm::vec3(inv_view[12], inv_view[13], inv_view[14]);

                        let globals = Globals {
                            view_proj: vp_array,
                            light_view_proj: lvp_array,
                            camera_pos: [cam_pos.x, cam_pos.y, cam_pos.z, 1.0],
                        };
                        queue.write_buffer(&globals_buffer, 0, bytemuck::bytes_of(&globals));

                        // Instances
                        let mut white_plain_instances = Vec::new();
                        let mut white_cf_instances = Vec::new();
                        let mut white_dga_instances = Vec::new();
                        let mut white_eb_instances = Vec::new();
                        let mut black_instances = Vec::new();
                        
                        let mut white_key_idx = 0;
                        for midi in 21..=108 {
                            let note_in_octave = midi % 12;
                            let is_black = matches!(note_in_octave, 1 | 3 | 6 | 8 | 10);
                            
                            if is_black {
                                // Align black keys centered on the gap (white_key_idx)
                                // Standard gap is at integer white_key_idx. 
                                // Mesh width is 0.96, gap is 0.04.
                                // Black key width is 0.58. 
                                // To center 0.58 on x=white_key_idx: offset = white_key_idx - 0.29
                                let x_offset = white_key_idx as f32 - 0.29;
                                black_instances.push(KeyInstance {
                                    offset: [x_offset, 2.3, 0.0],
                                    pressed: key_current[midi as usize],
                                });
                            } else {
                                let instance = KeyInstance {
                                    offset: [white_key_idx as f32, 0.0, 0.0],
                                    pressed: key_current[midi as usize],
                                };
                                
                                if midi == 21 || midi == 108 {
                                    white_plain_instances.push(instance);
                                } else {
                                    match note_in_octave {
                                        0 | 5 => white_cf_instances.push(instance),
                                        2 | 7 | 9 => white_dga_instances.push(instance),
                                        4 | 11 => white_eb_instances.push(instance),
                                        _ => (),
                                    }
                                }
                                white_key_idx += 1;
                            }
                        }

                        let plain_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&white_plain_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let cf_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&white_cf_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let dga_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&white_dga_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let eb_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&white_eb_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let black_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&black_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                        let frame = surface.get_current_texture().expect("failed to get texture");
                        let view = frame.texture.create_view(&TextureViewDescriptor::default());
                        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

                        {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("shadow_pass"),
                                color_attachments: &[],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                    view: &shadow_view,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                }),
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                            rpass.set_pipeline(&shadow_pipeline);
                            rpass.set_bind_group(0, &global_bind_group, &[]);

                            if !white_plain_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_plain.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, plain_buffer.slice(..));
                                rpass.set_index_buffer(white_plain.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_plain.index_count, 0, 0..white_plain_instances.len() as u32);
                            }

                            if !white_cf_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_cf.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, cf_buffer.slice(..));
                                rpass.set_index_buffer(white_cf.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_cf.index_count, 0, 0..white_cf_instances.len() as u32);
                            }
                            if !white_dga_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_dga.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, dga_buffer.slice(..));
                                rpass.set_index_buffer(white_dga.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_dga.index_count, 0, 0..white_dga_instances.len() as u32);
                            }
                            if !white_eb_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_eb.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, eb_buffer.slice(..));
                                rpass.set_index_buffer(white_eb.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_eb.index_count, 0, 0..white_eb_instances.len() as u32);
                            }
                            if !black_instances.is_empty() {
                                rpass.set_vertex_buffer(0, black_mesh.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, black_buffer.slice(..));
                                rpass.set_index_buffer(black_mesh.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..black_mesh.index_count, 0, 0..black_instances.len() as u32);
                            }
                        }

                        {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("main_pass"),
                                color_attachments: &[Some(RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: Operations {
                                        load: LoadOp::Clear(Color { r: 0.1, g: 0.11, b: 0.13, a: 1.0 }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                    depth_slice: None,
                                })],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                    view: &depth_texture,
                                    depth_ops: Some(wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(1.0),
                                        store: wgpu::StoreOp::Store,
                                    }),
                                    stencil_ops: None,
                                }),
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });

                            rpass.set_pipeline(&pipeline);
                            rpass.set_bind_group(0, &global_bind_group, &[]);
                            rpass.set_bind_group(1, &shadow_bind_group, &[]);

                            if !white_plain_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_plain.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, plain_buffer.slice(..));
                                rpass.set_index_buffer(white_plain.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_plain.index_count, 0, 0..white_plain_instances.len() as u32);
                            }

                            if !white_cf_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_cf.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, cf_buffer.slice(..));
                                rpass.set_index_buffer(white_cf.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_cf.index_count, 0, 0..white_cf_instances.len() as u32);
                            }
                            if !white_dga_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_dga.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, dga_buffer.slice(..));
                                rpass.set_index_buffer(white_dga.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_dga.index_count, 0, 0..white_dga_instances.len() as u32);
                            }
                            if !white_eb_instances.is_empty() {
                                rpass.set_vertex_buffer(0, white_eb.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, eb_buffer.slice(..));
                                rpass.set_index_buffer(white_eb.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..white_eb.index_count, 0, 0..white_eb_instances.len() as u32);
                            }
                            if !black_instances.is_empty() {
                                rpass.set_vertex_buffer(0, black_mesh.vertex_buffer.slice(..));
                                rpass.set_vertex_buffer(1, black_buffer.slice(..));
                                rpass.set_index_buffer(black_mesh.index_buffer.slice(..), IndexFormat::Uint32);
                                rpass.draw_indexed(0..black_mesh.index_count, 0, 0..black_instances.len() as u32);
                            }
                        }

                        queue.submit(once(encoder.finish()));
                        frame.present();
                    }

                    _ => (),
                },

                Event::AboutToWait => {
                    window.request_redraw();
                }

                _ => (),
            }
        })
        .unwrap();
}

fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
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
    let texture = device.create_texture(&desc);
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
