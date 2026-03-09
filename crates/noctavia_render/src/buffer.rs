use std::{marker::PhantomData, mem::size_of};

use bytemuck::Pod;
use rustc_hash::FxHashMap;
use wgpu::{VertexAttribute, VertexBufferLayout, VertexStepMode};

#[derive(Debug, Clone, Copy)]
pub enum BufferAttributeType {
    F32,
    U32,
    U16,
    I32,
    I16,
    U8,
    I8,
}

impl BufferAttributeType {
    pub fn size(&self) -> usize {
        match self {
            BufferAttributeType::F32 => 4,
            BufferAttributeType::U32 => 4,
            BufferAttributeType::U16 => 2,
            BufferAttributeType::I32 => 4,
            BufferAttributeType::I16 => 2,
            BufferAttributeType::U8 => 1,
            BufferAttributeType::I8 => 1,
        }
    }
}

impl BufferAttributeType {
    pub fn to_wgpu_format(&self, item_size: usize) -> wgpu::VertexFormat {
        match (self, item_size) {
            (BufferAttributeType::F32, 1) => wgpu::VertexFormat::Float32,
            (BufferAttributeType::F32, 2) => wgpu::VertexFormat::Float32x2,
            (BufferAttributeType::F32, 3) => wgpu::VertexFormat::Float32x3,
            (BufferAttributeType::F32, 4) => wgpu::VertexFormat::Float32x4,

            (BufferAttributeType::U32, 1) => wgpu::VertexFormat::Uint32,
            (BufferAttributeType::U32, 2) => wgpu::VertexFormat::Uint32x2,
            (BufferAttributeType::U32, 3) => wgpu::VertexFormat::Uint32x3,
            (BufferAttributeType::U32, 4) => wgpu::VertexFormat::Uint32x4,

            _ => panic!("Unsupported attribute format"),
        }
    }
}

#[derive(Debug)]
pub struct BufferAttribute<T: Pod + Copy> {
    location: u32,
    name: String,
    data: Vec<u8>,
    item_size: usize,
    attribute_type: BufferAttributeType,
    _phantom: PhantomData<T>,
}

impl<T: Pod + Copy> BufferAttribute<T> {
    pub fn new(
        name: impl Into<String>,
        location: u32,
        array: &[T],
        item_size: usize,
        attribute_type: BufferAttributeType,
    ) -> Self {
        let byte_len = array.len() * size_of::<T>();

        let data =
            unsafe { std::slice::from_raw_parts(array.as_ptr() as *const u8, byte_len).to_vec() };

        Self {
            location,
            name: name.into(),
            data,
            item_size,
            attribute_type,
            _phantom: PhantomData,
        }
    }

    pub fn attribute_type(&self) -> BufferAttributeType {
        self.attribute_type
    }

    pub fn item_size(&self) -> usize {
        self.item_size
    }

    pub fn count(&self) -> usize {
        self.data.len() / (self.attribute_type.size() * self.item_size)
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.data)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl<T: Pod> BufferAttribute<T> {
    pub fn upload_attribute(
        &self,
        device: &wgpu::Device,
        usage: wgpu::BufferUsages,
    ) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("GPU Attribute"),
            contents: &self.as_bytes(),
            usage,
        })
    }
}

#[derive(Debug)]
pub struct BufferGeometry {
    attributes: FxHashMap<String, BufferAttribute>,
    index: Option<BufferAttribute>,
}

impl BufferGeometry {
    pub fn new() -> Self {
        Self {
            attributes: FxHashMap::default(),
            index: None,
        }
    }

    /// Add or replace attribute
    pub fn set_attribute(&mut self, attribute: BufferAttribute) {
        self.attributes.insert(attribute.name.clone(), attribute);
    }

    /// Get attribute by name
    pub fn get_attribute(&self, name: &str) -> Option<&BufferAttribute> {
        self.attributes.get(name)
    }

    /// Mutable attribute access
    pub fn get_attribute_mut(&mut self, name: &str) -> Option<&mut BufferAttribute> {
        self.attributes.get_mut(name)
    }

    /// Set index buffer
    pub fn set_index(&mut self, attribute: BufferAttribute) {
        self.index = Some(attribute);
    }

    /// Get index buffer
    pub fn get_index(&self) -> Option<&BufferAttribute> {
        self.index.as_ref()
    }

    /// Vertex count (based on position attribute)
    pub fn vertex_count(&self) -> Option<usize> {
        self.get_attribute("position").map(|a| a.count())
    }

    pub fn layout(&self) -> VertexBufferLayout<'static> {
        let mut offset = 0;
        let mut attrs: Vec<VertexAttribute> = Vec::new();

        // sort by location to ensure stable layout
        let mut sorted_attrs: Vec<_> = self.attributes.values().collect();
        sorted_attrs.sort_by_key(|a| a.location);

        for attr in sorted_attrs {
            let format = attr.attribute_type.to_wgpu_format(attr.item_size);
            attrs.push(VertexAttribute {
                format,
                offset,
                shader_location: attr.location,
            });
            offset += (attr.item_size * attr.attribute_type.size()) as u64;
        }

        VertexBufferLayout {
            array_stride: offset,
            step_mode: VertexStepMode::Vertex,
            attributes: Box::leak(attrs.into_boxed_slice()),
        }
    }
}
