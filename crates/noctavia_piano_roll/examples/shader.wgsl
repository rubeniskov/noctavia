struct Globals {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var shadow_map: texture_depth_2d;
@group(1) @binding(1)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct InstanceInput {
    @location(3) offset: vec3<f32>,
    @location(4) pressed: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shadow_pos: vec4<f32>,
    @location(4) pressed: f32,
};

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    
    let pivot_y = 6.0;
    let angle = instance.pressed * 0.04;
    let s = sin(angle);
    let c = cos(angle);
    
    let rel_y = model.position.y - pivot_y;
    let rotated_y = pivot_y + rel_y * c - model.position.z * s;
    let rotated_z = rel_y * s + model.position.z * c;
    
    let local_pos = vec3<f32>(model.position.x, rotated_y, rotated_z);
    
    let rotated_normal = vec3<f32>(
        model.normal.x,
        model.normal.y * c - model.normal.z * s,
        model.normal.y * s + model.normal.z * c
    );

    let world_pos = local_pos + instance.offset;
    out.world_position = world_pos;
    out.clip_position = globals.view_proj * vec4<f32>(world_pos, 1.0);
    out.normal = rotated_normal;
    out.color = model.color;
    out.pressed = instance.pressed;
    out.shadow_pos = globals.light_view_proj * vec4<f32>(world_pos, 1.0);
    
    return out;
}

@vertex
fn vs_shadow(
    model: VertexInput,
    instance: InstanceInput,
) -> @builtin(position) vec4<f32> {
    let pivot_y = 6.0;
    let angle = instance.pressed * 0.04;
    let s = sin(angle);
    let c = cos(angle);
    let rel_y = model.position.y - pivot_y;
    let rotated_y = pivot_y + rel_y * c - model.position.z * s;
    let rotated_z = rel_y * s + model.position.z * c;
    let local_pos = vec3<f32>(model.position.x, rotated_y, rotated_z);
    let world_pos = local_pos + instance.offset;
    return globals.light_view_proj * vec4<f32>(world_pos, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.normal);
    let V = normalize(globals.camera_pos.xyz - in.world_position);
    
    // --- LIGHTING SETUP ---
    // 1. Key Light (Sun/Directional with Shadows)
    let L1 = normalize(vec3<f32>(0.3, -0.8, 1.0));
    let shadow_coords = in.shadow_pos.xyz / in.shadow_pos.w;
    let shadow = textureSampleCompare(shadow_map, shadow_sampler, shadow_coords.xy * vec2<f32>(0.5, -0.5) + 0.5, shadow_coords.z - 0.005);
    
    // 2. Fill Light (Soft from front-left)
    let L2 = normalize(vec3<f32>(-0.5, -0.5, 0.5));
    
    // 3. Back/Rim Light (Helps define edges)
    let L3 = normalize(vec3<f32>(0.0, 1.0, 0.2));

    // Material properties
    let is_black = in.color.r < 0.2;
    let shininess = select(64.0, 128.0, is_black);
    let spec_intensity = select(0.4, 0.8, is_black);
    let ambient_val = 0.15;

    // --- DIFFUSE ---
    let diff1 = max(dot(N, L1), 0.0) * shadow;
    let diff2 = max(dot(N, L2), 0.0) * 0.3;
    let diff3 = max(dot(N, L3), 0.0) * 0.2;
    
    // --- SPECULAR (Blinn-Phong) ---
    let H1 = normalize(L1 + V);
    let H2 = normalize(L2 + V);
    let spec1 = pow(max(dot(N, H1), 0.0), shininess) * spec_intensity * shadow;
    let spec2 = pow(max(dot(N, H2), 0.0), shininess) * spec_intensity * 0.5;

    // --- RIM LIGHTING (Fakes rounded edge "glow") ---
    let rim = pow(1.0 - max(dot(N, V), 0.0), 4.0) * 0.2;

    let press_darken = 1.0 - in.pressed * 0.1;
    let side_dim = select(1.0, 0.9, abs(N.x) > 0.8);
    
    let diffuse_total = (ambient_val + diff1 + diff2 + diff3);
    var final_rgb = in.color.rgb * diffuse_total * press_darken * side_dim + (spec1 + spec2 + rim);
    
    return vec4<f32>(final_rgb, in.color.a);
}
