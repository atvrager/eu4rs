// Vertex shader

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
    var out: VertexOutput;
    // Big Triangle Optimization
    // i=0 -> (-1, -1)
    // i=1 -> ( 3, -1)
    // i=2 -> (-1,  3)
    let x = f32(i32(in_vertex_index) << 1 & 2) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index) & 2) * 2.0 - 1.0;
    out.clip_position = vec4<f32>(x, -y, 0.0, 1.0); // Invert Y if needed for WGPU/Vulkan coords?
    out.tex_coords = vec2<f32>(x * 0.5 + 0.5, 0.5 + y * 0.5); // UVs
    return out;
}

// Fragment shader

struct CameraUniform {
    pos: vec2<f32>,
    inv_zoom: vec2<f32>,
};

@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var<uniform> camera: CameraUniform;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Transform UV to World Space
    let centered_uv = in.tex_coords - vec2<f32>(0.5, 0.5);
    let world_uv = camera.pos + centered_uv * camera.inv_zoom;

    // Wrap X axis
    let u = (world_uv.x % 1.0 + 1.0) % 1.0;
    
    // Check Y bounds (Black border)
    if (world_uv.y < 0.0 || world_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    // Use world_uv.y as is
    let final_uv = vec2<f32>(u, world_uv.y);

    return textureSample(t_diffuse, s_diffuse, final_uv);
}
