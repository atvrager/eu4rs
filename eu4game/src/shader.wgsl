// Vertex shader - Big Triangle Optimization

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Big Triangle: covers entire screen with one triangle
    // i=0 -> (-1, -1), i=1 -> (3, -1), i=2 -> (-1, 3)
    let x = f32(i32(in_vertex_index) << 1 & 2) * 2.0 - 1.0;
    let y = f32(i32(in_vertex_index) & 2) * 2.0 - 1.0;
    out.clip_position = vec4<f32>(x, -y, 0.0, 1.0);
    out.tex_coords = vec2<f32>(x * 0.5 + 0.5, 0.5 + y * 0.5);
    return out;
}

// Fragment shader - GPU Political Map Rendering

struct CameraUniform {
    pos: vec2<f32>,
    inv_zoom: vec2<f32>,
};

struct MapSettings {
    texture_size: vec2<f32>,    // Province texture dimensions
    lookup_size: f32,           // Lookup texture width (e.g., 8192)
    border_enabled: f32,        // 1.0 = show borders, 0.0 = hide
};

// Province ID texture (RG8 encoded: R = low byte, G = high byte)
@group(0) @binding(0)
var t_province: texture_2d<f32>;
@group(0) @binding(1)
var s_province: sampler;

// Color lookup texture (1D: province_id -> RGBA color)
@group(0) @binding(2)
var t_lookup: texture_2d<f32>;
@group(0) @binding(3)
var s_lookup: sampler;

@group(0) @binding(4)
var<uniform> camera: CameraUniform;

@group(0) @binding(5)
var<uniform> settings: MapSettings;

// Heightmap texture for terrain shading
@group(0) @binding(6)
var t_heightmap: texture_2d<f32>;
@group(0) @binding(7)
var s_heightmap: sampler;

// Decode province ID from RG channels (R = low byte, G = high byte)
fn decode_province_id(color: vec4<f32>) -> u32 {
    let low = u32(color.r * 255.0 + 0.5);
    let high = u32(color.g * 255.0 + 0.5);
    return low + (high << 8u);
}

// Sample province ID at a UV coordinate
fn sample_province_id(uv: vec2<f32>) -> u32 {
    let color = textureSample(t_province, s_province, uv);
    return decode_province_id(color);
}

// Look up color for a province ID
fn lookup_color(province_id: u32) -> vec4<f32> {
    // Lookup texture is 1D (Nx1), sample at x = province_id / lookup_size
    let u = (f32(province_id) + 0.5) / settings.lookup_size;
    return textureSample(t_lookup, s_lookup, vec2<f32>(u, 0.5));
}

// Check if this pixel is on a province border
fn is_border(uv: vec2<f32>, center_id: u32) -> bool {
    let pixel_size = 1.0 / settings.texture_size;

    // Sample 4 neighbors
    let left_id = sample_province_id(uv + vec2<f32>(-pixel_size.x, 0.0));
    let right_id = sample_province_id(uv + vec2<f32>(pixel_size.x, 0.0));
    let up_id = sample_province_id(uv + vec2<f32>(0.0, -pixel_size.y));
    let down_id = sample_province_id(uv + vec2<f32>(0.0, pixel_size.y));

    return left_id != center_id || right_id != center_id ||
           up_id != center_id || down_id != center_id;
}

// Compute terrain shading from heightmap using directional lighting
// Returns a multiplier in range [0.6, 1.4] to darken/lighten the base color
fn compute_terrain_shading(uv: vec2<f32>) -> f32 {
    let pixel_size = 1.0 / settings.texture_size;

    // Sample heightmap at current position and neighbors
    let h_center = textureSample(t_heightmap, s_heightmap, uv).r;
    let h_left = textureSample(t_heightmap, s_heightmap, uv + vec2<f32>(-pixel_size.x, 0.0)).r;
    let h_right = textureSample(t_heightmap, s_heightmap, uv + vec2<f32>(pixel_size.x, 0.0)).r;
    let h_up = textureSample(t_heightmap, s_heightmap, uv + vec2<f32>(0.0, -pixel_size.y)).r;
    let h_down = textureSample(t_heightmap, s_heightmap, uv + vec2<f32>(0.0, pixel_size.y)).r;

    // Compute gradient (approximates surface normal)
    let dx = (h_right - h_left) * 0.5;
    let dy = (h_down - h_up) * 0.5;

    // Light direction: from upper-left (NW), simulating sun position
    // Negative X (from left), negative Y (from top)
    let light_dir = normalize(vec3<f32>(-0.5, -0.7, 0.5));

    // Approximate surface normal from gradient
    // Scale gradient to control shading intensity
    let gradient_scale = 8.0;
    let normal = normalize(vec3<f32>(-dx * gradient_scale, -dy * gradient_scale, 1.0));

    // Lambertian diffuse lighting
    let diffuse = max(dot(normal, light_dir), 0.0);

    // Add ambient to prevent pure black shadows
    let ambient = 0.5;
    let shading = ambient + diffuse * 0.5;

    // Also add subtle height-based tinting (higher = slightly lighter)
    let height_boost = (h_center - 0.3) * 0.15;

    return clamp(shading + height_boost, 0.6, 1.3);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Transform UV to world space using camera
    let centered_uv = in.tex_coords - vec2<f32>(0.5, 0.5);
    let world_uv = camera.pos + centered_uv * camera.inv_zoom;

    // Wrap X axis for seamless world navigation
    let u = (world_uv.x % 1.0 + 1.0) % 1.0;

    // Black border for Y out of bounds
    if (world_uv.y < 0.0 || world_uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let final_uv = vec2<f32>(u, world_uv.y);

    // Sample province ID
    let province_id = sample_province_id(final_uv);

    // Look up the color for this province
    var color = lookup_color(province_id);

    // Apply terrain shading from heightmap
    let terrain_shade = compute_terrain_shading(final_uv);
    color = vec4<f32>(color.rgb * terrain_shade, color.a);

    // Apply border darkening if enabled
    if (settings.border_enabled > 0.5 && is_border(final_uv, province_id)) {
        color = vec4<f32>(0.08, 0.08, 0.08, 1.0);
    }

    return color;
}

// =============================================================================
// Army Marker Instanced Rendering
// =============================================================================

struct ArmyInstance {
    @location(0) world_pos: vec2<f32>,  // Position in UV space (0..1)
    @location(1) color: vec4<f32>,       // Marker color (RGBA)
};

struct ArmyVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,  // For diamond shape
};

// Camera uniform for army shader (separate bind group, binding 0)
@group(0) @binding(0)
var<uniform> army_camera: CameraUniform;

@vertex
fn vs_army(
    @builtin(vertex_index) vertex_index: u32,
    instance: ArmyInstance,
) -> ArmyVertexOutput {
    var out: ArmyVertexOutput;

    // Quad vertices (2 triangles, 6 vertices) - SQUARE
    var positions = array<vec2<f32>, 6>(
        // First triangle
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(1.0, -1.0),   // bottom-right
        vec2<f32>(1.0, 1.0),    // top-right
        // Second triangle
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(1.0, 1.0),    // top-right
        vec2<f32>(-1.0, 1.0),   // top-left
    );

    let local_pos = positions[vertex_index];
    out.local_pos = local_pos;

    // Transform army world position to clip space first
    let centered = instance.world_pos - army_camera.pos;
    let center_clip = centered / army_camera.inv_zoom * vec2<f32>(2.0, -2.0);

    // Scale marker with zoom level (smaller when zoomed out, larger when zoomed in)
    // inv_zoom.y is larger when zoomed out, smaller when zoomed in
    // Default inv_zoom.y is ~0.36 (map fills screen), zoomed in might be ~0.1, zoomed out ~1.0
    let zoom_scale = clamp(0.15 / army_camera.inv_zoom.y, 0.3, 2.0);

    // Account for aspect ratio to make square (1920/1080 ≈ 1.78)
    let aspect = 1.78;
    let base_size = 0.02;  // Base size at default zoom
    let screen_size = base_size * zoom_scale;
    let screen_offset = vec2<f32>(
        local_pos.x * screen_size / aspect,
        local_pos.y * screen_size
    );

    out.clip_position = vec4<f32>(
        center_clip.x + screen_offset.x,
        center_clip.y + screen_offset.y,
        0.0,
        1.0
    );
    out.color = instance.color;

    return out;
}

@fragment
fn fs_army(in: ArmyVertexOutput) -> @location(0) vec4<f32> {
    // Square with black border
    let edge = max(abs(in.local_pos.x), abs(in.local_pos.y));

    if (edge > 0.75) {
        // Black border
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    return in.color;
}

// =============================================================================
// Fleet Marker Instanced Rendering (Diamond Shape)
// =============================================================================

struct FleetInstance {
    @location(0) world_pos: vec2<f32>,  // Position in UV space (0..1)
    @location(1) color: vec4<f32>,       // Marker color (RGBA)
};

struct FleetVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,  // For diamond shape
};

@vertex
fn vs_fleet(
    @builtin(vertex_index) vertex_index: u32,
    instance: FleetInstance,
) -> FleetVertexOutput {
    var out: FleetVertexOutput;

    // Same quad vertices as army (actual shape determined in fragment shader)
    var positions = array<vec2<f32>, 6>(
        // First triangle
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(1.0, -1.0),   // bottom-right
        vec2<f32>(1.0, 1.0),    // top-right
        // Second triangle
        vec2<f32>(-1.0, -1.0),  // bottom-left
        vec2<f32>(1.0, 1.0),    // top-right
        vec2<f32>(-1.0, 1.0),   // top-left
    );

    let local_pos = positions[vertex_index];
    out.local_pos = local_pos;

    // Transform fleet world position to clip space
    let centered = instance.world_pos - army_camera.pos;
    let center_clip = centered / army_camera.inv_zoom * vec2<f32>(2.0, -2.0);

    // Scale marker with zoom level (same as army)
    let zoom_scale = clamp(0.15 / army_camera.inv_zoom.y, 0.3, 2.0);

    // Account for aspect ratio
    let aspect = 1.78;
    let base_size = 0.02;
    let screen_size = base_size * zoom_scale;
    let screen_offset = vec2<f32>(
        local_pos.x * screen_size / aspect,
        local_pos.y * screen_size
    );

    out.clip_position = vec4<f32>(
        center_clip.x + screen_offset.x,
        center_clip.y + screen_offset.y,
        0.0,
        1.0
    );
    out.color = instance.color;

    return out;
}

@fragment
fn fs_fleet(in: FleetVertexOutput) -> @location(0) vec4<f32> {
    // Diamond shape: check if inside diamond (|x| + |y| <= 1)
    let diamond_dist = abs(in.local_pos.x) + abs(in.local_pos.y);

    if (diamond_dist > 1.0) {
        // Outside diamond - transparent
        discard;
    }

    if (diamond_dist > 0.75) {
        // Black border
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    return in.color;
}

// =============================================================================
// UI Sprite Rendering (for flags, icons, etc.)
// =============================================================================

struct SpriteVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

// Sprite texture
@group(0) @binding(0)
var t_sprite: texture_2d<f32>;
@group(0) @binding(1)
var s_sprite: sampler;

@vertex
fn vs_sprite(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,      // Screen position (clip space)
    @location(1) size: vec2<f32>,     // Size in clip space
) -> SpriteVertexOutput {
    var out: SpriteVertexOutput;

    // Quad vertices (2 triangles, 6 vertices)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),  // bottom-left
        vec2<f32>(1.0, 0.0),  // bottom-right
        vec2<f32>(1.0, 1.0),  // top-right
        vec2<f32>(0.0, 0.0),  // bottom-left
        vec2<f32>(1.0, 1.0),  // top-right
        vec2<f32>(0.0, 1.0),  // top-left
    );

    let local_pos = positions[vertex_index];

    // Position in clip space
    out.clip_position = vec4<f32>(
        pos.x + local_pos.x * size.x,
        pos.y - local_pos.y * size.y,  // Y goes down in clip space
        0.0,
        1.0
    );

    // Texture coordinates (no flip - TGA origin is bottom-left, image crate handles it)
    out.tex_coords = vec2<f32>(local_pos.x, local_pos.y);

    return out;
}

@fragment
fn fs_sprite(in: SpriteVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_sprite, s_sprite, in.tex_coords);
}
