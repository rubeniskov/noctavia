use std::borrow::Cow;

use wgpu::{
    ColorTargetState,
    Device,
    FragmentState,
    MultisampleState,
    PipelineLayoutDescriptor,
    PolygonMode,
    PrimitiveState,
    RenderPipeline,
    RenderPipelineDescriptor,
    ShaderModuleDescriptor,
    ShaderSource,
    TextureFormat,
    VertexState,
};

use crate::{
    instance::InstanceData,
    mesh::Vertex,
};

pub struct Pipelines {
    pub filled: RenderPipeline,
    pub wireframe: Option<RenderPipeline>,
}

impl Pipelines {
    pub fn new(
        device: &Device,
        shader_src: &str,
        surface_format: TextureFormat,
        supports_wireframe: bool,
    ) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("main_shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(shader_src)),
        });

        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let filled = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("filled_pipeline"),
            layout: Some(&layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        });

        let wireframe = if supports_wireframe {
            Some(device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("wireframe_pipeline"),
                layout: Some(&layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::layout()],
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState {
                    polygon_mode: PolygonMode::Line,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview: None,
            }))
        } else {
            None
        };

        Self { filled, wireframe }
    }
}