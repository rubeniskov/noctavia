use crate::primitive::Mesh;

pub struct DebugTools;

impl DebugTools {
    pub fn generate_axes_mesh() -> Mesh {
        Mesh::axes()
    }

    pub fn generate_pivot_mesh() -> Mesh {
        Mesh::pivot_marker()
    }

    pub fn generate_vertex_dot_mesh() -> Mesh {
        Mesh::vertex_marker()
    }

    pub fn generate_normals_mesh(mesh: &Mesh) -> Mesh {
        mesh.generate_normals_mesh()
    }
}
