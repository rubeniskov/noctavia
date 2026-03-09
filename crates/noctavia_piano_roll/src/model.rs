use bytemuck::{Pod, Zeroable};
use wgpu::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Vertex {
    position: [f32; 2],
}

impl Vertex {
    fn new(x: f32, y: f32) -> Self {
        Self { position: [x, y] }
    }

    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &[VertexAttribute {
                format: VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        }
    }
}

type Index = u16;

#[derive(Debug, Clone, Copy)]
struct MeshRange {
    first_index: u32,
    index_count: u32,
    base_vertex: i32,
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct KeyInstance {
    offset: [f32; 2],   // x/y piano position
    size: [f32; 2],     // width/height
    color: [f32; 4],    // optional
}


impl KeyInstance {
    fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<KeyInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                VertexAttribute {
                    shader_location: 1,
                    format: VertexFormat::Float32x2,
                    offset: 0,
                },
                VertexAttribute {
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                    offset: size_of::<[f32; 2]>() as u64,
                },
                VertexAttribute {
                    shader_location: 3,
                    format: VertexFormat::Float32x4,
                    offset: size_of::<[f32; 4]>() as u64,
                },
            ],
        }
    }
}