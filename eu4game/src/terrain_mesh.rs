//! Terrain mesh generation for 3D map rendering.
//!
//! Generates chunked terrain meshes from the heightmap for GPU rendering.
//! The map is divided into chunks to enable frustum culling and efficient
//! rendering of only visible portions.

use glam::Vec3;

/// Vertex for terrain mesh rendering.
///
/// Layout: position (3 × f32) + tex_coords (2 × f32) = 20 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    /// World-space position (X, Y, Z). Y is height, set to 0 initially
    /// and displaced by heightmap in vertex shader.
    pub position: [f32; 3],
    /// Texture coordinates for sampling province/terrain maps (0.0-1.0).
    pub tex_coords: [f32; 2],
}

impl TerrainVertex {
    /// Expected size in bytes for GPU buffer alignment.
    pub const SIZE: usize = 20;

    /// Creates a vertex layout description for wgpu.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::SIZE as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: vec3<f32>
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // tex_coords: vec2<f32>
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Axis-aligned bounding box for frustum culling.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    /// Minimum corner (x, y, z).
    pub min: Vec3,
    /// Maximum corner (x, y, z).
    pub max: Vec3,
}

impl Aabb {
    /// Creates a new AABB from min and max corners.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Creates an AABB that encompasses all given points (alternative constructor).
    #[allow(dead_code)]
    pub fn from_points(points: impl Iterator<Item = Vec3>) -> Self {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for p in points {
            min = min.min(p);
            max = max.max(p);
        }

        Self { min, max }
    }
}

/// A chunk of terrain mesh data.
#[derive(Debug)]
pub struct TerrainChunk {
    /// Vertices for this chunk.
    pub vertices: Vec<TerrainVertex>,
    /// Indices for triangle list rendering.
    pub indices: Vec<u32>,
    /// Bounding box for frustum culling.
    pub aabb: Aabb,
    /// Chunk position in chunk grid (col, row).
    pub chunk_pos: (u32, u32),
}

/// Configuration for terrain mesh generation.
#[derive(Debug, Clone, Copy)]
pub struct TerrainMeshConfig {
    /// Map width in world units.
    pub map_width: f32,
    /// Map height in world units.
    pub map_height: f32,
    /// Number of vertices per chunk side (e.g., 128 means 128×128 grid).
    pub vertices_per_chunk_side: u32,
    /// Number of chunks horizontally.
    pub chunks_x: u32,
    /// Number of chunks vertically (Z direction).
    pub chunks_z: u32,
}

impl TerrainMeshConfig {
    /// Default configuration matching EU4's 5632×2048 map.
    pub const EU4_DEFAULT: Self = Self {
        map_width: 5632.0,
        map_height: 2048.0,
        vertices_per_chunk_side: 128,
        chunks_x: 11,
        chunks_z: 4,
    };

    /// Width of a single chunk in world units.
    pub fn chunk_width(&self) -> f32 {
        self.map_width / self.chunks_x as f32
    }

    /// Height (Z-dimension) of a single chunk in world units.
    pub fn chunk_height(&self) -> f32 {
        self.map_height / self.chunks_z as f32
    }

    /// Total number of chunks.
    pub fn total_chunks(&self) -> u32 {
        self.chunks_x * self.chunks_z
    }

    /// Vertices per chunk.
    pub fn vertices_per_chunk(&self) -> u32 {
        self.vertices_per_chunk_side * self.vertices_per_chunk_side
    }

    /// Indices per chunk (6 per quad: 2 triangles × 3 vertices).
    pub fn indices_per_chunk(&self) -> u32 {
        let quads_per_side = self.vertices_per_chunk_side - 1;
        quads_per_side * quads_per_side * 6
    }
}

/// Generates a terrain chunk at the specified grid position.
///
/// # Arguments
/// * `config` - Terrain mesh configuration
/// * `chunk_x` - Chunk column (0 to chunks_x - 1)
/// * `chunk_z` - Chunk row (0 to chunks_z - 1)
pub fn generate_chunk(config: &TerrainMeshConfig, chunk_x: u32, chunk_z: u32) -> TerrainChunk {
    let chunk_width = config.chunk_width();
    let chunk_height = config.chunk_height();
    let n = config.vertices_per_chunk_side;

    // World-space origin of this chunk
    let origin_x = chunk_x as f32 * chunk_width;
    let origin_z = chunk_z as f32 * chunk_height;

    // Generate vertices
    let mut vertices = Vec::with_capacity(config.vertices_per_chunk() as usize);
    let mut min_pos = Vec3::splat(f32::MAX);
    let mut max_pos = Vec3::splat(f32::MIN);

    for z in 0..n {
        for x in 0..n {
            // Normalize within chunk (0.0 to 1.0)
            let u_local = x as f32 / (n - 1) as f32;
            let v_local = z as f32 / (n - 1) as f32;

            // World position
            let world_x = origin_x + u_local * chunk_width;
            let world_z = origin_z + v_local * chunk_height;

            // Texture coordinates (global UV across entire map)
            let tex_u = world_x / config.map_width;
            let tex_v = world_z / config.map_height;

            let pos = Vec3::new(world_x, 0.0, world_z);
            min_pos = min_pos.min(pos);
            max_pos = max_pos.max(pos);

            vertices.push(TerrainVertex {
                position: [world_x, 0.0, world_z],
                tex_coords: [tex_u, tex_v],
            });
        }
    }

    // Generate indices (two triangles per quad)
    let quads_per_side = n - 1;
    let mut indices = Vec::with_capacity(config.indices_per_chunk() as usize);

    for z in 0..quads_per_side {
        for x in 0..quads_per_side {
            // Vertex indices for this quad
            let top_left = z * n + x;
            let top_right = top_left + 1;
            let bottom_left = top_left + n;
            let bottom_right = bottom_left + 1;

            // First triangle (top-left, bottom-left, top-right)
            indices.push(top_left);
            indices.push(bottom_left);
            indices.push(top_right);

            // Second triangle (top-right, bottom-left, bottom-right)
            indices.push(top_right);
            indices.push(bottom_left);
            indices.push(bottom_right);
        }
    }

    // Extend AABB height to account for potential heightmap displacement
    // The actual height will be determined by the heightmap texture
    max_pos.y = 255.0; // Maximum possible height value

    TerrainChunk {
        vertices,
        indices,
        aabb: Aabb::new(min_pos, max_pos),
        chunk_pos: (chunk_x, chunk_z),
    }
}

/// Generates all terrain chunks for the map.
pub fn generate_all_chunks(config: &TerrainMeshConfig) -> Vec<TerrainChunk> {
    let mut chunks = Vec::with_capacity(config.total_chunks() as usize);

    for z in 0..config.chunks_z {
        for x in 0..config.chunks_x {
            chunks.push(generate_chunk(config, x, z));
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_vertex_size() {
        // TerrainVertex must be exactly 20 bytes for GPU buffer alignment
        assert_eq!(
            std::mem::size_of::<TerrainVertex>(),
            20,
            "TerrainVertex should be 20 bytes (3×f32 + 2×f32)"
        );
        assert_eq!(
            TerrainVertex::SIZE,
            std::mem::size_of::<TerrainVertex>(),
            "SIZE constant should match actual size"
        );
    }

    #[test]
    fn test_generate_chunk_vertex_count() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunk = generate_chunk(&config, 0, 0);

        // 128×128 = 16384 vertices per chunk
        assert_eq!(
            chunk.vertices.len(),
            16384,
            "Chunk should have 128×128 = 16384 vertices"
        );
        assert_eq!(
            config.vertices_per_chunk(),
            16384,
            "Config should report 16384 vertices per chunk"
        );
    }

    #[test]
    fn test_generate_chunk_index_count() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunk = generate_chunk(&config, 0, 0);

        // (128-1)×(128-1)×6 = 127×127×6 = 96774 indices
        let expected = 127 * 127 * 6;
        assert_eq!(
            chunk.indices.len(),
            expected,
            "Chunk should have (n-1)²×6 = {} indices",
            expected
        );
        assert_eq!(
            config.indices_per_chunk(),
            expected as u32,
            "Config should report {} indices per chunk",
            expected
        );
    }

    #[test]
    fn test_chunk_aabb_bounds() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunk = generate_chunk(&config, 0, 0);

        // First chunk should start at origin
        assert!(
            chunk.aabb.min.x.abs() < 0.001,
            "Chunk 0,0 min.x should be ~0, got {}",
            chunk.aabb.min.x
        );
        assert!(
            chunk.aabb.min.z.abs() < 0.001,
            "Chunk 0,0 min.z should be ~0, got {}",
            chunk.aabb.min.z
        );

        // Max should be at chunk boundary
        let expected_width = config.chunk_width();
        let expected_height = config.chunk_height();
        assert!(
            (chunk.aabb.max.x - expected_width).abs() < 0.001,
            "Chunk 0,0 max.x should be ~{}, got {}",
            expected_width,
            chunk.aabb.max.x
        );
        assert!(
            (chunk.aabb.max.z - expected_height).abs() < 0.001,
            "Chunk 0,0 max.z should be ~{}, got {}",
            expected_height,
            chunk.aabb.max.z
        );

        // All vertices should be within AABB (ignoring Y which is extended for heightmap)
        for v in &chunk.vertices {
            let pos = Vec3::from(v.position);
            assert!(
                pos.x >= chunk.aabb.min.x && pos.x <= chunk.aabb.max.x,
                "Vertex X {} outside AABB [{}, {}]",
                pos.x,
                chunk.aabb.min.x,
                chunk.aabb.max.x
            );
            assert!(
                pos.z >= chunk.aabb.min.z && pos.z <= chunk.aabb.max.z,
                "Vertex Z {} outside AABB [{}, {}]",
                pos.z,
                chunk.aabb.min.z,
                chunk.aabb.max.z
            );
        }
    }

    #[test]
    fn test_chunk_uv_mapping() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunk = generate_chunk(&config, 0, 0);

        // First vertex (top-left of first chunk)
        let first = &chunk.vertices[0];
        assert!(
            first.tex_coords[0].abs() < 0.001,
            "First vertex U should be ~0, got {}",
            first.tex_coords[0]
        );
        assert!(
            first.tex_coords[1].abs() < 0.001,
            "First vertex V should be ~0, got {}",
            first.tex_coords[1]
        );

        // Last vertex in first row
        let n = config.vertices_per_chunk_side as usize;
        let last_in_row = &chunk.vertices[n - 1];
        let expected_u = config.chunk_width() / config.map_width;
        assert!(
            (last_in_row.tex_coords[0] - expected_u).abs() < 0.001,
            "Last-in-row vertex U should be ~{}, got {}",
            expected_u,
            last_in_row.tex_coords[0]
        );

        // Last vertex (bottom-right of first chunk)
        let last = chunk.vertices.last().unwrap();
        let expected_u = config.chunk_width() / config.map_width;
        let expected_v = config.chunk_height() / config.map_height;
        assert!(
            (last.tex_coords[0] - expected_u).abs() < 0.001,
            "Last vertex U should be ~{}, got {}",
            expected_u,
            last.tex_coords[0]
        );
        assert!(
            (last.tex_coords[1] - expected_v).abs() < 0.001,
            "Last vertex V should be ~{}, got {}",
            expected_v,
            last.tex_coords[1]
        );
    }

    #[test]
    fn test_chunk_layout_coverage() {
        let config = TerrainMeshConfig::EU4_DEFAULT;

        // 11×4 = 44 chunks total
        assert_eq!(config.total_chunks(), 44, "Should have 11×4 = 44 chunks");

        // Verify all chunks together cover the full map
        let chunks = generate_all_chunks(&config);
        assert_eq!(chunks.len(), 44, "Should generate 44 chunks");

        // Find global AABB of all chunks
        let mut global_min = Vec3::splat(f32::MAX);
        let mut global_max = Vec3::splat(f32::MIN);

        for chunk in &chunks {
            global_min = global_min.min(chunk.aabb.min);
            global_max = global_max.max(chunk.aabb.max);
        }

        // Global coverage should match map dimensions
        assert!(
            global_min.x.abs() < 0.001,
            "Global min.x should be ~0, got {}",
            global_min.x
        );
        assert!(
            global_min.z.abs() < 0.001,
            "Global min.z should be ~0, got {}",
            global_min.z
        );
        assert!(
            (global_max.x - config.map_width).abs() < 0.001,
            "Global max.x should be ~{}, got {}",
            config.map_width,
            global_max.x
        );
        assert!(
            (global_max.z - config.map_height).abs() < 0.001,
            "Global max.z should be ~{}, got {}",
            config.map_height,
            global_max.z
        );
    }

    #[test]
    fn test_chunk_position_tracking() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunks = generate_all_chunks(&config);

        // Verify chunk positions are correctly set
        for (idx, chunk) in chunks.iter().enumerate() {
            let expected_x = (idx as u32) % config.chunks_x;
            let expected_z = (idx as u32) / config.chunks_x;
            assert_eq!(
                chunk.chunk_pos,
                (expected_x, expected_z),
                "Chunk {} should have position ({}, {})",
                idx,
                expected_x,
                expected_z
            );
        }
    }

    #[test]
    fn test_indices_form_valid_triangles() {
        let config = TerrainMeshConfig::EU4_DEFAULT;
        let chunk = generate_chunk(&config, 0, 0);

        // All indices should be within vertex bounds
        let vertex_count = chunk.vertices.len() as u32;
        for (i, &idx) in chunk.indices.iter().enumerate() {
            assert!(
                idx < vertex_count,
                "Index {} at position {} exceeds vertex count {}",
                idx,
                i,
                vertex_count
            );
        }

        // Indices should come in groups of 3 (triangles)
        assert_eq!(
            chunk.indices.len() % 3,
            0,
            "Index count should be divisible by 3"
        );
    }
}
