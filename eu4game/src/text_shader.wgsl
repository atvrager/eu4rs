// Text rendering shader - renders textured quads from glyph atlas

struct TextVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
};

// Glyph atlas texture
@group(0) @binding(0)
var t_atlas: texture_2d<f32>;
@group(0) @binding(1)
var s_atlas: sampler;

@vertex
fn vs_text(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,       // Clip space position (top-left)
    @location(1) size: vec2<f32>,      // Clip space size
    @location(2) uv_min: vec2<f32>,    // UV top-left
    @location(3) uv_max: vec2<f32>,    // UV bottom-right
    @location(4) color: vec4<f32>,     // Text color
) -> TextVertexOutput {
    var out: TextVertexOutput;

    // Quad vertices (2 triangles, 6 vertices)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 0.0),  // top-right
        vec2<f32>(1.0, 1.0),  // bottom-right
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 1.0),  // bottom-right
        vec2<f32>(0.0, 1.0),  // bottom-left
    );

    let local_pos = positions[vertex_index];

    // Position in clip space
    out.clip_position = vec4<f32>(
        pos.x + local_pos.x * size.x,
        pos.y - local_pos.y * size.y,  // Y goes down
        0.0,
        1.0
    );

    // Interpolate UVs
    out.tex_coords = vec2<f32>(
        mix(uv_min.x, uv_max.x, local_pos.x),
        mix(uv_min.y, uv_max.y, local_pos.y)
    );

    out.color = color;

    return out;
}

@fragment
fn fs_text(in: TextVertexOutput) -> @location(0) vec4<f32> {
    let atlas_sample = textureSample(t_atlas, s_atlas, in.tex_coords);
    // Use atlas alpha as mask, apply text color
    return vec4<f32>(in.color.rgb, in.color.a * atlas_sample.a);
}
