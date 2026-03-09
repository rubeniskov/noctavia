use std::mem::size_of;
use bytemuck_derive::{Pod, Zeroable};
use wgpu::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct KeyInstance {
    pub offset: [f32; 3],
    pub pressed: f32, // 0.0 to 1.0
}

impl KeyInstance {
    pub fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<KeyInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &[
                VertexAttribute {
                    shader_location: 3,
                    format: VertexFormat::Float32x3,
                    offset: 0,
                },
                VertexAttribute {
                    shader_location: 4,
                    format: VertexFormat::Float32,
                    offset: size_of::<[f32; 3]>() as u64,
                },
            ],
        }
    }
}
