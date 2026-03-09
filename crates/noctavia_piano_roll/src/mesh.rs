use noctavia_render::{add_vec3, normalize, normalize_vec3, Mesh};

/// Append a quad to given vertices and indices.
/// `v0..v3` are the four corner positions of the quad.
/// `vertices` and `indices` are the mesh buffers to fill.
/// Assumes counter-clockwise winding.
pub fn quad(
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u32>,
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
    v3: [f32; 3],
) {
    let base = vertices.len() as u32;
    vertices.push(v0);
    vertices.push(v1);
    vertices.push(v2);
    vertices.push(v3);

    indices.push(base);
    indices.push(base + 1);
    indices.push(base + 2);
    indices.push(base);
    indices.push(base + 2);
    indices.push(base + 3);
}

pub fn generate_white_key(notch_right: f32, notchleft: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();

    let width = 0.96;
    let height = 0.6;
    let depth = 6.0;
    let lip_height = 0.12;
    let lip_depth = 0.12;
    let notch_depth_ratio = 0.5;

    // --- Define vertices with intuitive variables ---
    // Top face
    let top_front_right = [0.0, 0.0, height];
    let top_front_right_idx = 0;
    let top_front_left = [width, 0.0, height];
    let top_front_left_idx = 1;
    let top_back_left = [width, depth, height];
    let top_back_left_idx = 2;
    let top_back_right = [0.0, depth, height];
    let top_back_right_idx = 3;

    // Bottom vertices
    let bottom_front_right = [0.0, 0.0, 0.0];
    let bottom_front_right_idx = 4;
    let bottom_front_left = [width, 0.0, 0.0];
    let bottom_front_left_idx = 5;
    let bottom_back_left = [width, depth, 0.0];
    let bottom_back_left_idx = 6;
    let bottom_back_right = [0.0, depth, 0.0];
    let bottom_back_right_idx = 7;

    // Lip vertices
    let lip_front_top_right = [0.0, 0.0 - lip_depth, height];
    let lip_front_top_right_idx = 8;
    let lip_front_top_left = [width, 0.0 - lip_depth, height];
    let lip_front_top_left_idx = 9;
    let lip_front_bottom_right = [0.0, -lip_depth, height - lip_height];
    let lip_front_bottom_right_idx = 10;
    let lip_front_bottom_left = [width, -lip_depth, height - lip_height]; 
    let lip_front_bottom_left_idx = 11;
    let lip_back_bottom_right = [0.0, 0.0, height - lip_height];
    let lip_back_bottom_right_idx = 12;
    let lip_back_bottom_left = [width, 0.0, height - lip_height];
    let lip_back_bottom_left_idx = 13;

        // Notch vertices
    let notch_top_back_left = [width, depth * notch_depth_ratio, height];
    let notch_top_back_left_idx = 14;
    let notch_top_back_right = [0.0, depth * notch_depth_ratio, height]; 
    let notch_top_back_right_idx = 15;
    let notch_bottom_back_left = [width, depth * notch_depth_ratio, 0.0];
    let notch_bottom_back_left_idx = 16;
    let notch_bottom_back_right = [0.0, depth * notch_depth_ratio, 0.0];
    let notch_bottom_back_right_idx = 17;

    // Push all vertices to positions vec
    positions.extend_from_slice(&[
        top_front_right,
        top_front_left,
        top_back_left,
        top_back_right,
        bottom_front_right,
        bottom_front_left,
        bottom_back_left,
        bottom_back_right,
        lip_front_top_right,
        lip_front_top_left,
        lip_front_bottom_right,
        lip_front_bottom_left,
        lip_back_bottom_right,
        lip_back_bottom_left,
        notch_top_back_left,
        notch_top_back_right,
        notch_bottom_back_left,
        notch_bottom_back_right,
    ]);

    // --- Indices for faces (CCW winding) ---
    // Top quad (+Z)
    indices.extend_from_slice(&[
        lip_front_top_right_idx,
        top_back_left_idx,  //notch_top_back_left_idx,
        top_back_right_idx, //notch_top_back_right_idx,
        top_back_left_idx,  //notch_top_back_left_idx,
        lip_front_top_right_idx,
        lip_front_top_left_idx,
    ]);

    // Lip faces
    indices.extend_from_slice(&[
        // Front quad (+Z)
        lip_front_top_right_idx,
        lip_front_bottom_left_idx,
        lip_front_top_left_idx,
        lip_front_top_right_idx,
        lip_front_bottom_right_idx,
        lip_front_bottom_left_idx,
        // Bottom quad (-Z)
        lip_front_bottom_right_idx,
        lip_back_bottom_right_idx,
        lip_front_bottom_left_idx,
        lip_back_bottom_right_idx,
        lip_back_bottom_left_idx,
        lip_front_bottom_left_idx,
        // Side quad (-X)
        lip_front_bottom_right_idx,
        lip_front_top_right_idx,
        lip_back_bottom_right_idx,
        top_front_right_idx,
        lip_back_bottom_right_idx,
        lip_front_top_right_idx,
        // Side quad (+X)
        lip_front_bottom_left_idx,
        lip_back_bottom_left_idx,
        lip_front_top_left_idx,
        top_front_left_idx,
        lip_front_top_left_idx,
        lip_back_bottom_left_idx,
    ]);

    // Front quad (Y=0)
    indices.extend_from_slice(&[
        bottom_front_left_idx,
        lip_back_bottom_left_idx,
        bottom_front_right_idx,
        bottom_front_right_idx,
        lip_back_bottom_left_idx,
        lip_back_bottom_right_idx,
    ]);

    // Side quad right
    indices.extend_from_slice(&[
        bottom_front_right_idx,
        top_front_right_idx,
        bottom_back_right_idx, // notch_bottom_back_right_idx,
        bottom_back_right_idx, // notch_bottom_back_right_idx,
        top_front_right_idx,
        top_back_right_idx, // notch_top_back_right_idx,
    ]);

    // Side quad left
    indices.extend_from_slice(&[
        top_front_left_idx,
        bottom_front_left_idx,
        bottom_back_left_idx, // notch_bottom_back_left_idx,
        bottom_back_left_idx,// notch_bottom_back_left_idx,
        top_back_left_idx, // notch_top_back_left_idx,
        top_front_left_idx,
    ]);

    // --- Convert positions to Vertex ---
    let vertices = positions
        .into_iter()
        .map(|position| noctavia_render::Vertex {
            position,
            normal: [0.0, 0.0, 1.0],     // placeholder, compute later
            color: [1.0, 1.0, 1.0, 1.0], // placeholder
        })
        .collect();

    noctavia_render::Mesh { vertices, indices }
}

pub fn generate_black_key() -> Mesh {
    let mut mesh = Mesh::new();
    let w = 0.58;
    let h = 3.7;
    let d = 0.5;
    let bz = 0.6;
    let tw = 0.08;
    let tf = 0.15;
    let color = [0.10, 0.10, 0.11, 1.0];
    let side_color = [0.05, 0.05, 0.06, 1.0];
    let b_bl = [0.0, 0.0, bz];
    let b_br = [w, 0.0, bz];
    let b_tr = [w, h, bz];
    let b_tl = [0.0, h, bz];
    let t_bl = [tw, tf, bz + d];
    let t_br = [w - tw, tf, bz + d];
    let t_tr = [w - tw, h, bz + d];
    let t_tl = [tw, h, bz + d];
    let n_up = [0.0, 0.0, 1.0];
    let n_dn = [0.0, 0.0, -1.0];
    let n_bk = [0.0, 1.0, 0.0];
    let n_fr = normalize([0.0, -d, tf]);
    let n_lt = normalize([-d, 0.0, tw]);
    let n_rt = normalize([d, 0.0, tw]);
    let n_t_bl = normalize_vec3(add_vec3(n_up, add_vec3(n_fr, n_lt)));
    let n_t_br = normalize_vec3(add_vec3(n_up, add_vec3(n_fr, n_rt)));
    let n_t_tr = normalize_vec3(add_vec3(n_up, add_vec3(n_bk, n_rt)));
    let n_t_tl = normalize_vec3(add_vec3(n_up, add_vec3(n_bk, n_lt)));
    mesh.add_quad_ext(
        [t_bl, t_br, t_tr, t_tl],
        [n_t_bl, n_t_br, n_t_tr, n_t_tl],
        color,
    );
    mesh.add_quad(b_bl, b_tl, b_tr, b_br, n_dn, side_color);
    mesh.add_quad_ext(
        [b_bl, b_br, t_br, t_bl],
        [n_fr, n_fr, n_t_br, n_t_bl],
        side_color,
    );
    mesh.add_quad_ext(
        [b_tr, b_tl, t_tl, t_tr],
        [n_bk, n_bk, n_t_tl, n_t_tr],
        side_color,
    );
    mesh.add_quad_ext(
        [b_tl, b_bl, t_bl, t_tl],
        [n_lt, n_lt, n_t_bl, n_t_tl],
        side_color,
    );
    mesh.add_quad_ext(
        [b_br, b_tr, t_tr, t_br],
        [n_rt, n_rt, n_t_tr, n_t_br],
        side_color,
    );
    mesh
}
