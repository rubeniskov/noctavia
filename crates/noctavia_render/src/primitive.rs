use bytemuck_derive::{Pod, Zeroable};
use std::mem::size_of;
use wgpu::{
    util::DeviceExt, BufferUsages, Device, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexStepMode,
};

#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], color: [f32; 4]) -> Self {
        Self {
            position,
            normal,
            color,
        }
    }

    pub fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: size_of::<[f32; 3]>() as u64,
                    shader_location: 1,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: (size_of::<[f32; 3]>() * 2) as u64,
                    shader_location: 2,
                },
            ],
        }
    }
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn add_quad(
        &mut self,
        p1: [f32; 3],
        p2: [f32; 3],
        p3: [f32; 3],
        p4: [f32; 3],
        normal: [f32; 3],
        color: [f32; 4],
    ) {
        self.add_quad_ext([p1, p2, p3, p4], [normal, normal, normal, normal], color);
    }

    pub fn add_quad_ext(&mut self, p: [[f32; 3]; 4], n: [[f32; 3]; 4], color: [f32; 4]) {
        let base_index = self.vertices.len() as u32;
        for i in 0..4 {
            self.vertices.push(Vertex::new(p[i], n[i], color));
        }
        self.indices.push(base_index);
        self.indices.push(base_index + 1);
        self.indices.push(base_index + 2);
        self.indices.push(base_index);
        self.indices.push(base_index + 2);
        self.indices.push(base_index + 3);
    }

    pub fn add_cylinder(&mut self, start: [f32; 3], end: [f32; 3], radius: f32, color: [f32; 4]) {
        let segments = 8;
        let diff = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
        let len = (diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2]).sqrt();
        if len < 0.0001 {
            return;
        }
        let dir = [diff[0] / len, diff[1] / len, diff[2] / len];
        let mut v1 = if dir[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let dot = v1[0] * dir[0] + v1[1] * dir[1] + v1[2] * dir[2];
        v1 = normalize([
            v1[0] - dot * dir[0],
            v1[1] - dot * dir[1],
            v1[2] - dot * dir[2],
        ]);
        let v2 = [
            dir[1] * v1[2] - dir[2] * v1[1],
            dir[2] * v1[0] - dir[0] * v1[2],
            dir[0] * v1[1] - dir[1] * v1[0],
        ];
        for i in 0..segments {
            let a1 = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let a2 = ((i + 1) as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let (c1, s1, c2, s2) = (a1.cos(), a1.sin(), a2.cos(), a2.sin());
            let p1 = [
                start[0] + (v1[0] * c1 + v2[0] * s1) * radius,
                start[1] + (v1[1] * c1 + v2[1] * s1) * radius,
                start[2] + (v1[2] * c1 + v2[2] * s1) * radius,
            ];
            let p2 = [
                start[0] + (v1[0] * c2 + v2[0] * s2) * radius,
                start[1] + (v1[1] * c2 + v2[1] * s2) * radius,
                start[2] + (v1[2] * c2 + v2[2] * s2) * radius,
            ];
            let p3 = [
                end[0] + (v1[0] * c2 + v2[0] * s2) * radius,
                end[1] + (v1[1] * c2 + v2[1] * s2) * radius,
                end[2] + (v1[2] * c2 + v2[2] * s2) * radius,
            ];
            let p4 = [
                end[0] + (v1[0] * c1 + v2[0] * s1) * radius,
                end[1] + (v1[1] * c1 + v2[1] * s1) * radius,
                end[2] + (v1[2] * c1 + v2[2] * s1) * radius,
            ];
            let n = normalize([
                v1[0] * ((c1 + c2) / 2.0) + v2[0] * ((s1 + s2) / 2.0),
                v1[1] * ((c1 + c2) / 2.0) + v2[1] * ((s1 + s2) / 2.0),
                v1[2] * ((c1 + c2) / 2.0) + v2[2] * ((s1 + s2) / 2.0),
            ]);
            self.add_quad(p1, p2, p3, p4, n, color);
        }
    }

    pub fn axes() -> Self {
        let mut mesh = Self::new();
        let len = 2.0;
        let r = 0.02;
        mesh.add_cylinder([0.0, 0.0, 0.0], [len, 0.0, 0.0], r, [1.0, 0.0, 0.0, 1.0]);
        mesh.add_cylinder([0.0, 0.0, 0.0], [0.0, len, 0.0], r, [0.0, 1.0, 0.0, 1.0]);
        mesh.add_cylinder([0.0, 0.0, 0.0], [0.0, 0.0, len], r, [0.0, 0.0, 1.0, 1.0]);
        mesh
    }

    pub fn pivot_marker() -> Self {
        let mut mesh = Self::new();
        let s = 0.05;
        let color = [1.0, 0.0, 1.0, 1.0];
        mesh.add_quad(
            [-s, -s, -s],
            [s, -s, -s],
            [s, s, -s],
            [-s, s, -s],
            [0.0, 0.0, -1.0],
            color,
        ); // Bot
        mesh.add_quad(
            [-s, -s, s],
            [s, -s, s],
            [s, s, s],
            [-s, s, s],
            [0.0, 0.0, 1.0],
            color,
        ); // Top
        mesh.add_quad(
            [-s, -s, -s],
            [s, -s, -s],
            [s, -s, s],
            [-s, -s, s],
            [0.0, -1.0, 0.0],
            color,
        ); // Front
        mesh.add_quad(
            [s, s, -s],
            [-s, s, -s],
            [-s, s, s],
            [s, s, s],
            [0.0, 1.0, 0.0],
            color,
        ); // Back
        mesh.add_quad(
            [-s, s, -s],
            [-s, -s, -s],
            [-s, -s, s],
            [-s, s, s],
            [-1.0, 0.0, 0.0],
            color,
        ); // Left
        mesh.add_quad(
            [s, -s, -s],
            [s, s, -s],
            [s, s, s],
            [s, -s, s],
            [1.0, 0.0, 0.0],
            color,
        ); // Right
        mesh
    }

    pub fn vertex_marker() -> Self {
        let mut mesh = Self::new();
        let s = 0.015;
        let color = [1.0, 0.5, 0.0, 1.0]; // Orange
        mesh.add_quad(
            [-s, -s, -s],
            [s, -s, -s],
            [s, s, -s],
            [-s, s, -s],
            [0.0, 0.0, -1.0],
            color,
        );
        mesh.add_quad(
            [-s, -s, s],
            [s, -s, s],
            [s, s, s],
            [-s, s, s],
            [0.0, 0.0, 1.0],
            color,
        );
        mesh.add_quad(
            [-s, -s, -s],
            [s, -s, -s],
            [s, -s, s],
            [-s, -s, s],
            [0.0, -1.0, 0.0],
            color,
        );
        mesh.add_quad(
            [s, s, -s],
            [-s, s, -s],
            [-s, s, s],
            [s, s, s],
            [0.0, 1.0, 0.0],
            color,
        );
        mesh.add_quad(
            [-s, s, -s],
            [-s, -s, -s],
            [-s, -s, s],
            [-s, s, s],
            [-1.0, 0.0, 0.0],
            color,
        );
        mesh.add_quad(
            [s, -s, -s],
            [s, s, -s],
            [s, s, s],
            [s, -s, s],
            [1.0, 0.0, 0.0],
            color,
        );
        mesh
    }

    pub fn generate_normals_mesh(&self) -> Self {
        let mut visualizer = Self::new();
        let line_len = 0.25;
        let line_radius = 0.004;
        let color = [1.0, 1.0, 0.0, 1.0];
        for i in (0..self.indices.len()).step_by(3) {
            let i1 = self.indices[i] as usize;
            let i2 = self.indices[i + 1] as usize;
            let i3 = self.indices[i + 2] as usize;
            let (v1, v2, v3) = (self.vertices[i1], self.vertices[i2], self.vertices[i3]);
            let center = [
                (v1.position[0] + v2.position[0] + v3.position[0]) / 3.0,
                (v1.position[1] + v2.position[1] + v3.position[1]) / 3.0,
                (v1.position[2] + v2.position[2] + v3.position[2]) / 3.0,
            ];
            let normal = normalize([
                (v1.normal[0] + v2.normal[0] + v3.normal[0]) / 3.0,
                (v1.normal[1] + v2.normal[1] + v3.normal[1]) / 3.0,
                (v1.normal[2] + v2.normal[2] + v3.normal[2]) / 3.0,
            ]);
            let end = [
                center[0] + normal[0] * line_len,
                center[1] + normal[1] * line_len,
                center[2] + normal[2] * line_len,
            ];
            visualizer.add_cylinder(center, end, line_radius, color);
        }
        visualizer
    }

    pub fn compute_vertext_normals(&mut self) {
        for i in (0..self.indices.len()).step_by(3) {
            let i1 = self.indices[i] as usize;
            let i2 = self.indices[i + 1] as usize;
            let i3 = self.indices[i + 2] as usize;

            let v1 = self.vertices[i1].position;
            let v2 = self.vertices[i2].position;
            let v3 = self.vertices[i3].position;

            // Compute edges
            let edge1 = [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]];
            let edge2 = [v3[0] - v1[0], v3[1] - v1[1], v3[2] - v1[2]];

            // Cross product
            let normal = normalize([
                edge1[1] * edge2[2] - edge1[2] * edge2[1],
                edge1[2] * edge2[0] - edge1[0] * edge2[2],
                edge1[0] * edge2[1] - edge1[1] * edge2[0],
            ]);

            // Assign same normal to all three vertices (flat shading)
            self.vertices[i1].normal = normal;
            self.vertices[i2].normal = normal;
            self.vertices[i3].normal = normal;
        }
    }
}

pub fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}
pub fn add_vec3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn normalize_vec3(v: [f32; 3]) -> [f32; 3] {
    normalize(v)
}

pub struct GpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl GpuMesh {
    pub fn from_mesh(device: &Device, mesh: &Mesh) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_vertex_buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh_index_buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        }
    }
}
