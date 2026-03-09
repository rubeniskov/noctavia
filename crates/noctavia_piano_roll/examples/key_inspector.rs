use std::{borrow::Cow, iter::once, mem::size_of, sync::Arc};
use bytemuck_derive::{Pod, Zeroable};
use wgpu::{
    LoadOp, MultisampleState, Operations,
    RenderPassColorAttachment, RenderPassDescriptor, 
    TextureViewDescriptor, IndexFormat,
    util::DeviceExt,
};
use winit::{
    event::{ElementState, Event, WindowEvent, MouseButton, MouseScrollDelta},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowAttributes,
};
use cryoglyph::{
    Attrs, Buffer, Cache, Color as TextColor, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use nalgebra_glm as glm;

use noctavia_piano_roll::instance::KeyInstance;
use noctavia_render::{RenderEngine, OrbitalCamera, DebugTools, Mesh, Vertex, GpuMesh};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct Globals {
    view_proj: [f32; 16],
    light_view_proj: [f32; 16],
    camera_pos: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum KeyType {
    WhitePlain,
    WhiteCF,
    WhiteDGA,
    WhiteEB,
    Black,
    Full,
}

fn main() {
    println!("========================================");
    println!("   NOCTAVIA PIANO KEY INSPECTOR");
    println!("========================================");
    println!("  0 (º): Select Plain White key");
    println!("  1: Select White C/F key");
    println!("  2: Select White D/G/A key");
    println!("  3: Select White E/B key");
    println!("  4: Select Black key");
    println!("  F: Select Full Keyboard");
    println!("  W: Toggle Wireframe");
    println!("  C: Cycle Culling (None -> Front -> Back)");
    println!("  X: Toggle World Axes");
    println!("  P: Toggle Pivot Point");
    println!("  N: Toggle Normals");
    println!("  V: Toggle Vertices");
    println!("  Space: Press key (lever action)");
    println!("  Mouse Left Drag: Orbit camera");
    println!("  Mouse Wheel: Zoom in/out");
    println!("========================================");

    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        #[allow(deprecated)]
        event_loop
            .create_window(
                WindowAttributes::default()
                    .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
                    .with_title("Noctavia Key Inspector"),
            )
            .expect("failed to create a window"),
    );

    let mut engine = RenderEngine::new(window.clone());
    let swapchain_format = engine.config.format;

    let shader_src = include_str!("shader.wgsl");
    let shader = engine.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
    });

    // --- BIND GROUP LAYOUTS ---
    let global_bind_group_layout = engine.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

    let shadow_bind_group_layout = engine.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

    let pipeline_layout = engine.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&global_bind_group_layout, &shadow_bind_group_layout],
        push_constant_ranges: &[],
    });

    // --- PIPELINES GENERATOR ---
    let create_pipeline = |device: &wgpu::Device, cull: Option<wgpu::Face>, wireframe: bool, depth: bool| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("inspector_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), KeyInstance::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(swapchain_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: cull,
                polygon_mode: if wireframe { wgpu::PolygonMode::Line } else { wgpu::PolygonMode::Fill },
                ..Default::default()
            },
            depth_stencil: if depth { Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }) } else { None },
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    };

    let pipeline_none = create_pipeline(&engine.device, None, false, true);
    let pipeline_back = create_pipeline(&engine.device, Some(wgpu::Face::Back), false, true);
    let pipeline_front = create_pipeline(&engine.device, Some(wgpu::Face::Front), false, true);
    let wire_none = create_pipeline(&engine.device, None, true, true);
    let wire_back = create_pipeline(&engine.device, Some(wgpu::Face::Back), true, true);
    let wire_front = create_pipeline(&engine.device, Some(wgpu::Face::Front), true, true);
    let helper_pipeline = create_pipeline(&engine.device, None, false, false);

    // --- MESHES ---
    let white_plain_mesh = noctavia_piano_roll::mesh::generate_white_key(0.0, 0.0);
    let white_cf_mesh = noctavia_piano_roll::mesh::generate_white_key(0.0, 0.35);

    let white_dga_mesh = noctavia_piano_roll::mesh::generate_white_key(0.35, 0.35);
    let white_eb_mesh = noctavia_piano_roll::mesh::generate_white_key(0.35, 0.0);
    let black_key_mesh = noctavia_piano_roll::mesh::generate_black_key();

    let white_plain = GpuMesh::from_mesh(&engine.device, &white_plain_mesh);
    let white_cf = GpuMesh::from_mesh(&engine.device, &white_cf_mesh);
    let white_dga = GpuMesh::from_mesh(&engine.device, &white_dga_mesh);
    let white_eb = GpuMesh::from_mesh(&engine.device, &white_eb_mesh);
    let black_mesh = GpuMesh::from_mesh(&engine.device, &black_key_mesh);
    
    let white_plain_normals = GpuMesh::from_mesh(&engine.device, &white_plain_mesh.generate_normals_mesh());
    let white_cf_normals = GpuMesh::from_mesh(&engine.device, &white_cf_mesh.generate_normals_mesh());
    let white_dga_normals = GpuMesh::from_mesh(&engine.device, &white_dga_mesh.generate_normals_mesh());
    let white_eb_normals = GpuMesh::from_mesh(&engine.device, &white_eb_mesh.generate_normals_mesh());
    let black_normals = GpuMesh::from_mesh(&engine.device, &black_key_mesh.generate_normals_mesh());
    
    let axes_mesh = GpuMesh::from_mesh(&engine.device, &DebugTools::generate_axes_mesh());
    let pivot_mesh = GpuMesh::from_mesh(&engine.device, &DebugTools::generate_pivot_mesh());
    let vertex_dot_mesh = GpuMesh::from_mesh(&engine.device, &DebugTools::generate_vertex_dot_mesh());

    // --- UNIFORMS ---
    let globals_buffer = engine.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("globals_buffer"),
        size: size_of::<Globals>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let global_bind_group = engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("global_bind_group"),
        layout: &global_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: globals_buffer.as_entire_binding(),
        }],
    });

    let dummy_shadow_texture = engine.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("dummy_shadow"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let dummy_shadow_view = dummy_shadow_texture.create_view(&TextureViewDescriptor::default());
    let dummy_shadow_sampler = engine.device.create_sampler(&wgpu::SamplerDescriptor {
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    });
    let shadow_bind_group = engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_bind_group"),
        layout: &shadow_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&dummy_shadow_view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&dummy_shadow_sampler) },
        ],
    });

    let mut depth_texture = engine.create_depth_texture();

    // --- TEXT RENDERING ---
    let mut font_system = FontSystem::new();
    let mut swash_cache = SwashCache::new();
    let cache = Cache::new(&engine.device);
    let mut atlas = TextAtlas::new(&engine.device, &engine.queue, &cache, swapchain_format);
    let mut text_renderer = TextRenderer::new(&mut atlas, &engine.device, MultisampleState::default(), None);
    let mut viewport = Viewport::new(&engine.device, &cache);
    viewport.update(&engine.queue, Resolution { width: engine.config.width, height: engine.config.height });

    let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(18.0, 24.0));
    text_buffer.set_size(&mut font_system, Some(engine.config.width as f32), Some(engine.config.height as f32));

    // --- INSPECTOR STATE ---
    let mut current_key = KeyType::WhitePlain;
    let mut wireframe_enabled = false;
    let mut axes_enabled = true;
    let mut pivot_enabled = true;
    let mut normals_enabled = false;
    let mut vertices_enabled = false;
    let mut cull_mode = 1; // 0: None, 1: Front, 2: Back
    let mut pressed_factor = 0.0f32;
    let mut camera = OrbitalCamera::new(glm::vec3(0.48, 3.0, 0.3), 10.0);
    let mut mouse_pressed = false;
    let mut last_mouse_pos = glm::vec2(0.0, 0.0);
    
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
                        engine.resize(size.width, size.height);
                        depth_texture = engine.create_depth_texture();
                        text_buffer.set_size(&mut font_system, Some(engine.config.width as f32), Some(engine.config.height as f32));
                        viewport.update(&engine.queue, Resolution { width: size.width, height: size.height });
                    }

                    WindowEvent::KeyboardInput { event, .. } => {
                        if let PhysicalKey::Code(key_code) = event.physical_key {
                            if event.state == ElementState::Pressed {
                                match key_code {
                                    KeyCode::Backquote => { current_key = KeyType::WhitePlain; camera.center = glm::vec3(0.48, 3.0, 0.3); camera.radius = 10.0; },
                                    KeyCode::Digit1 => { current_key = KeyType::WhiteCF; camera.center = glm::vec3(0.48, 3.0, 0.3); camera.radius = 10.0; },
                                    KeyCode::Digit2 => { current_key = KeyType::WhiteDGA; camera.center = glm::vec3(0.48, 3.0, 0.3); camera.radius = 10.0; },
                                    KeyCode::Digit3 => { current_key = KeyType::WhiteEB; camera.center = glm::vec3(0.48, 3.0, 0.3); camera.radius = 10.0; },
                                    KeyCode::Digit4 => { current_key = KeyType::Black; camera.center = glm::vec3(0.29, 4.15, 0.85); camera.radius = 10.0; },
                                    KeyCode::KeyF => { current_key = KeyType::Full; camera.center = glm::vec3(26.0, 3.0, 0.3); camera.radius = 30.0; },
                                    KeyCode::KeyW => { wireframe_enabled = !wireframe_enabled; },
                                    KeyCode::KeyC => { cull_mode = (cull_mode + 1) % 3; },
                                    KeyCode::KeyX => { axes_enabled = !axes_enabled; },
                                    KeyCode::KeyP => { pivot_enabled = !pivot_enabled; },
                                    KeyCode::KeyN => { normals_enabled = !normals_enabled; },
                                    KeyCode::KeyV => { vertices_enabled = !vertices_enabled; },
                                    KeyCode::Space => pressed_factor = 1.0,
                                    _ => (),
                                }
                            } else {
                                if key_code == KeyCode::Space {
                                    pressed_factor = 0.0;
                                }
                            }
                        }
                    }

                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Left {
                            mouse_pressed = state == ElementState::Pressed;
                        }
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        let pos = glm::vec2(position.x as f32, position.y as f32);
                        if mouse_pressed {
                            let delta = pos - last_mouse_pos;
                            camera.orbit(delta.x * 0.01, delta.y * 0.01);
                        }
                        last_mouse_pos = pos;
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        let amount = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y * 0.5,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
                        };
                        camera.zoom(amount);
                    }

                    WindowEvent::RedrawRequested => {
                        let key_name = match current_key {
                            KeyType::WhitePlain => "Plain White",
                            KeyType::WhiteCF => "White C/F",
                            KeyType::WhiteDGA => "White D/G/A",
                            KeyType::WhiteEB => "White E/B",
                            KeyType::Black => "Black",
                            KeyType::Full => "Full Keyboard",
                        };
                        let cull_name = match cull_mode {
                            0 => "None",
                            1 => "Front (CCW)",
                            2 => "Back (CW)",
                            _ => "Unknown",
                        };
                        text_buffer.set_text(&mut font_system, &format!(
                                "NOCTAVIA KEY INSPECTOR\n\n\
                                 [º] Plain White Key\n\
                                 [1] White C/F\n\
                                 [2] White D/G/A\n\
                                 [3] White E/B\n\
                                 [4] Black\n\
                                 [F] Full Keyboard\n\n\
                                 Selected: {}\n\
                                 Wireframe: {}\n\
                                 Culling: {}\n\
                                 Axes: {}\n\
                                 Pivot: {}\n\
                                 Normals: {}\n\
                                 Vertices: {}\n\n\
                                 [W] Toggle Wireframe\n\
                                 [C] Cycle Culling\n\
                                 [X] Toggle Axes\n\
                                 [P] Toggle Pivot\n\
                                 [N] Toggle Normals\n\
                                 [V] Toggle Vertices\n\
                                 [Space] Lever action\n\
                                 [Mouse Drag] Orbit\n\
                                 [Mouse Wheel] Zoom",
                                key_name, if wireframe_enabled { "ON" } else { "OFF" }, cull_name,
                                if axes_enabled { "ON" } else { "OFF" }, if pivot_enabled { "ON" } else { "OFF" },
                                if normals_enabled { "ON" } else { "OFF" }, if vertices_enabled { "ON" } else { "OFF" }
                            ), &Attrs::new().family(Family::SansSerif), Shaping::Advanced, None);
                        text_buffer.shape_until_scroll(&mut font_system, false);

                        let eye = camera.eye_position();
                        let view = camera.view_matrix();
                        let projection = camera.projection_matrix(engine.config.width as f32 / engine.config.height as f32);
                        
                        let view_proj = projection * view;
                        let mut vp_array = [0.0f32; 16];
                        vp_array.copy_from_slice(glm::value_ptr(&view_proj));
                        
                        let globals = Globals {
                            view_proj: vp_array,
                            light_view_proj: vp_array,
                            camera_pos: [eye.x, eye.y, eye.z, 1.0],
                        };
                        engine.queue.write_buffer(&globals_buffer, 0, bytemuck::bytes_of(&globals));

                        // Instances
                        let mut white_plain_instances = Vec::new();
                        let mut white_cf_instances = Vec::new();
                        let mut white_dga_instances = Vec::new();
                        let mut white_eb_instances = Vec::new();
                        let mut black_instances = Vec::new();
                        let mut pivot_instances = Vec::new();
                        let mut vertex_instances = Vec::new();

                        let collect_mesh_vertices = |m: &Mesh, base_offset: [f32; 3], target: &mut Vec<KeyInstance>| {
                            use std::collections::HashSet;
                            let mut seen = HashSet::new();
                            for v in &m.vertices {
                                let key = ( (v.position[0]*1000.0) as i32, (v.position[1]*1000.0) as i32, (v.position[2]*1000.0) as i32 );
                                if seen.insert(key) {
                                    target.push(KeyInstance {
                                        offset: [v.position[0] + base_offset[0], v.position[1] + base_offset[1], v.position[2] + base_offset[2]],
                                        pressed: 0.0,
                                    });
                                }
                            }
                        };

                        match current_key {
                            KeyType::Full => {
                                let mut white_key_idx = 0;
                                for midi in 21..=108 {
                                    let note_in_octave = midi % 12;
                                    let is_black = matches!(note_in_octave, 1 | 3 | 6 | 8 | 10);
                                    if is_black {
                                        let x_offset = white_key_idx as f32 - 0.29;
                                        let offset = [x_offset, 2.3, 0.0];
                                        black_instances.push(KeyInstance { offset, pressed: pressed_factor });
                                        if pivot_enabled { pivot_instances.push(KeyInstance { offset: [x_offset + 0.29, 6.0, 0.0], pressed: 0.0 }); }
                                        if vertices_enabled { collect_mesh_vertices(&black_key_mesh, offset, &mut vertex_instances); }
                                    } else {
                                        let offset = [white_key_idx as f32, 0.0, 0.0];
                                        let instance = KeyInstance { offset, pressed: pressed_factor };
                                        let mesh = if midi == 21 || midi == 108 { white_plain_instances.push(instance); &white_plain_mesh }
                                        else {
                                            match note_in_octave {
                                                0 | 5 => { white_cf_instances.push(instance); &white_cf_mesh },
                                                2 | 7 | 9 => { white_dga_instances.push(instance); &white_dga_mesh },
                                                4 | 11 => { white_eb_instances.push(instance); &white_eb_mesh },
                                                _ => &white_plain_mesh,
                                            }
                                        };
                                        if pivot_enabled { pivot_instances.push(KeyInstance { offset: [white_key_idx as f32 + 0.48, 6.0, 0.0], pressed: 0.0 }); }
                                        if vertices_enabled { collect_mesh_vertices(mesh, offset, &mut vertex_instances); }
                                        white_key_idx += 1;
                                    }
                                }
                            }
                            KeyType::WhitePlain => {
                                white_plain_instances.push(KeyInstance { offset: [0.0, 0.0, 0.0], pressed: pressed_factor });
                                if pivot_enabled { pivot_instances.push(KeyInstance { offset: [0.48, 6.0, 0.0], pressed: 0.0 }); }
                                if vertices_enabled { collect_mesh_vertices(&white_plain_mesh, [0.0, 0.0, 0.0], &mut vertex_instances); }
                            }
                            KeyType::WhiteCF => {
                                white_cf_instances.push(KeyInstance { offset: [0.0, 0.0, 0.0], pressed: pressed_factor });
                                if pivot_enabled { pivot_instances.push(KeyInstance { offset: [0.48, 6.0, 0.0], pressed: 0.0 }); }
                                if vertices_enabled { collect_mesh_vertices(&white_cf_mesh, [0.0, 0.0, 0.0], &mut vertex_instances); }
                            }
                            KeyType::WhiteDGA => {
                                white_dga_instances.push(KeyInstance { offset: [0.0, 0.0, 0.0], pressed: pressed_factor });
                                if pivot_enabled { pivot_instances.push(KeyInstance { offset: [0.48, 6.0, 0.0], pressed: 0.0 }); }
                                if vertices_enabled { collect_mesh_vertices(&white_dga_mesh, [0.0, 0.0, 0.0], &mut vertex_instances); }
                            }
                            KeyType::WhiteEB => {
                                white_eb_instances.push(KeyInstance { offset: [0.0, 0.0, 0.0], pressed: pressed_factor });
                                if pivot_enabled { pivot_instances.push(KeyInstance { offset: [0.48, 6.0, 0.0], pressed: 0.0 }); }
                                if vertices_enabled { collect_mesh_vertices(&white_eb_mesh, [0.0, 0.0, 0.0], &mut vertex_instances); }
                            }
                            KeyType::Black => {
                                black_instances.push(KeyInstance { offset: [0.0, 2.3, 0.0], pressed: pressed_factor });
                                if pivot_enabled { pivot_instances.push(KeyInstance { offset: [0.29, 6.0, 0.6], pressed: 0.0 }); }
                                if vertices_enabled { collect_mesh_vertices(&black_key_mesh, [0.0, 2.3, 0.0], &mut vertex_instances); }
                            }
                        }

                        let plain_buffer: Option<wgpu::Buffer> = if !white_plain_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&white_plain_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let cf_buffer: Option<wgpu::Buffer> = if !white_cf_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&white_cf_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let dga_buffer: Option<wgpu::Buffer> = if !white_dga_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&white_dga_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let eb_buffer: Option<wgpu::Buffer> = if !white_eb_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&white_eb_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let black_buffer: Option<wgpu::Buffer> = if !black_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&black_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let pivot_buffer: Option<wgpu::Buffer> = if !pivot_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&pivot_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let vertex_dots_buffer: Option<wgpu::Buffer> = if !vertex_instances.is_empty() { Some(engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&vertex_instances), usage: wgpu::BufferUsages::VERTEX })) } else { None };
                        let origin_instance_buffer = engine.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&[KeyInstance { offset: [0.0, 0.0, 0.0], pressed: 0.0 }]), usage: wgpu::BufferUsages::VERTEX });

                        let frame = engine.surface.get_current_texture().expect("failed to get texture");
                        let view = frame.texture.create_view(&TextureViewDescriptor::default());
                        let mut encoder = engine.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                        text_renderer.prepare(&engine.device, &engine.queue, &mut encoder, &mut font_system, &mut atlas, &viewport, [TextArea { buffer: &text_buffer, left: 20.0, top: 20.0, scale: 1.0, bounds: TextBounds { left: 0, top: 0, right: engine.config.width as i32, bottom: engine.config.height as i32 }, default_color: TextColor::rgb(255, 255, 255) }], &mut swash_cache).unwrap();

                        {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("inspector_pass"),
                                color_attachments: &[Some(RenderPassColorAttachment { view: &view, resolve_target: None, ops: Operations { load: LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.07, a: 1.0 }), store: wgpu::StoreOp::Store }, depth_slice: None })],
                                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &depth_texture, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                                timestamp_writes: None, occlusion_query_set: None,
                            });

                            let active_pipeline = match (wireframe_enabled, cull_mode) {
                                (false, 0) => &pipeline_none, (false, 1) => &pipeline_front, (false, 2) => &pipeline_back,
                                (true, 0) => &wire_none, (true, 1) => &wire_front, (true, 2) => &wire_back,
                                _ => &pipeline_none,
                            };

                            rpass.set_pipeline(active_pipeline);
                            rpass.set_bind_group(0, &global_bind_group, &[]);
                            rpass.set_bind_group(1, &shadow_bind_group, &[]);

                            if let Some(buf) = &plain_buffer { rpass.set_vertex_buffer(0, white_plain.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(white_plain.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_plain.index_count, 0, 0..white_plain_instances.len() as u32); }
                            if let Some(buf) = &cf_buffer { rpass.set_vertex_buffer(0, white_cf.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(white_cf.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_cf.index_count, 0, 0..white_cf_instances.len() as u32); }
                            if let Some(buf) = &dga_buffer { rpass.set_vertex_buffer(0, white_dga.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(white_dga.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_dga.index_count, 0, 0..white_dga_instances.len() as u32); }
                            if let Some(buf) = &eb_buffer { rpass.set_vertex_buffer(0, white_eb.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(white_eb.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_eb.index_count, 0, 0..white_eb_instances.len() as u32); }
                            if let Some(buf) = &black_buffer { rpass.set_vertex_buffer(0, black_mesh.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(black_mesh.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..black_mesh.index_count, 0, 0..black_instances.len() as u32); }
                        }

                        if axes_enabled || pivot_enabled || normals_enabled || vertices_enabled {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("helper_pass"),
                                color_attachments: &[Some(RenderPassColorAttachment { view: &view, resolve_target: None, ops: Operations { load: LoadOp::Load, store: wgpu::StoreOp::Store }, depth_slice: None })],
                                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
                            });
                            rpass.set_pipeline(&helper_pipeline);
                            rpass.set_bind_group(0, &global_bind_group, &[]);
                            rpass.set_bind_group(1, &shadow_bind_group, &[]);
                            if axes_enabled { rpass.set_vertex_buffer(0, axes_mesh.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, origin_instance_buffer.slice(..)); rpass.set_index_buffer(axes_mesh.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..axes_mesh.index_count, 0, 0..1); }
                            if let Some(buf) = &pivot_buffer { rpass.set_vertex_buffer(0, pivot_mesh.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(pivot_mesh.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..pivot_mesh.index_count, 0, 0..pivot_instances.len() as u32); }
                            if let Some(buf) = &vertex_dots_buffer { rpass.set_vertex_buffer(0, vertex_dot_mesh.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(vertex_dot_mesh.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..vertex_dot_mesh.index_count, 0, 0..vertex_instances.len() as u32); }
                            if normals_enabled {
                                let (mesh, inst_buf, count) = match current_key {
                                    KeyType::WhitePlain => (&white_plain_normals, &plain_buffer, white_plain_instances.len()),
                                    KeyType::WhiteCF => (&white_cf_normals, &cf_buffer, white_cf_instances.len()),
                                    KeyType::WhiteDGA => (&white_dga_normals, &dga_buffer, white_dga_instances.len()),
                                    KeyType::WhiteEB => (&white_eb_normals, &eb_buffer, white_eb_instances.len()),
                                    KeyType::Black => (&black_normals, &black_buffer, black_instances.len()),
                                    KeyType::Full => (&white_cf_normals, &cf_buffer, white_cf_instances.len()),
                                };
                                if let Some(buf) = inst_buf { rpass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, buf.slice(..)); rpass.set_index_buffer(mesh.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..mesh.index_count, 0, 0..count as u32);
                                    if current_key == KeyType::Full {
                                        if let Some(b) = &dga_buffer { rpass.set_vertex_buffer(0, white_dga_normals.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, b.slice(..)); rpass.set_index_buffer(white_dga_normals.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_dga_normals.index_count, 0, 0..white_dga_instances.len() as u32); }
                                        if let Some(b) = &eb_buffer { rpass.set_vertex_buffer(0, white_eb_normals.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, b.slice(..)); rpass.set_index_buffer(white_eb_normals.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_eb_normals.index_count, 0, 0..white_eb_instances.len() as u32); }
                                        if let Some(b) = &black_buffer { rpass.set_vertex_buffer(0, black_normals.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, b.slice(..)); rpass.set_index_buffer(black_normals.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..black_normals.index_count, 0, 0..black_instances.len() as u32); }
                                        if let Some(b) = &plain_buffer { rpass.set_vertex_buffer(0, white_plain_normals.vertex_buffer.slice(..)); rpass.set_vertex_buffer(1, b.slice(..)); rpass.set_index_buffer(white_plain_normals.index_buffer.slice(..), IndexFormat::Uint32); rpass.draw_indexed(0..white_plain_normals.index_count, 0, 0..white_plain_instances.len() as u32); }
                                    }
                                }
                            }
                        }

                        {
                            let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
                                label: Some("text_pass"),
                                color_attachments: &[Some(RenderPassColorAttachment { view: &view, resolve_target: None, ops: Operations { load: LoadOp::Load, store: wgpu::StoreOp::Store }, depth_slice: None })],
                                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None,
                            });
                            text_renderer.render(&atlas, &viewport, &mut rpass).unwrap();
                        }
                        engine.queue.submit(once(encoder.finish()));
                        frame.present();
                    }
                    _ => (),
                },
                Event::AboutToWait => { window.request_redraw(); }
                _ => (),
            }
        })
        .unwrap();
}

