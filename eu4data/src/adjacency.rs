use crate::cache::{CacheError, CacheableResource};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub type ProvinceId = u32;

/// RGB color representation for province mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Graph of province adjacencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjacencyGraph {
    /// Map of province ID to list of adjacent province IDs
    adjacencies: HashMap<ProvinceId, HashSet<ProvinceId>>,
}

impl AdjacencyGraph {
    /// Create a new empty adjacency graph.
    pub fn new() -> Self {
        Self {
            adjacencies: HashMap::new(),
        }
    }

    /// Add a bidirectional adjacency between two provinces.
    pub fn add_adjacency(&mut self, p1: ProvinceId, p2: ProvinceId) {
        self.adjacencies.entry(p1).or_default().insert(p2);
        self.adjacencies.entry(p2).or_default().insert(p1);
    }

    /// Get all neighbors of a province.
    pub fn neighbors(&self, province: ProvinceId) -> Vec<ProvinceId> {
        self.adjacencies
            .get(&province)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if two provinces are adjacent.
    pub fn are_adjacent(&self, p1: ProvinceId, p2: ProvinceId) -> bool {
        self.adjacencies
            .get(&p1)
            .map(|set| set.contains(&p2))
            .unwrap_or(false)
    }

    /// Get total number of provinces in the graph.
    pub fn province_count(&self) -> usize {
        self.adjacencies.len()
    }
}

impl Default for AdjacencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry from adjacencies.csv (special straits/sea crossings).
#[derive(Debug, Clone)]
pub struct StraitEntry {
    pub from: ProvinceId,
    pub to: ProvinceId,
    pub strait_type: String,
    pub through: Option<ProvinceId>,
    pub start_x: i32,
    pub start_y: i32,
    pub stop_x: i32,
    pub stop_y: i32,
    pub adjacency_rule_name: Option<String>,
    pub comment: Option<String>,
}

/// Load straits from adjacencies.csv.
///
/// Format: From;To;Type;Through;start_x;start_y;stop_x;stop_y;adjacency_rule_name;Comment
pub fn load_adjacencies_csv(path: &Path) -> Result<Vec<StraitEntry>, CacheError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .comment(Some(b'#'))
        .from_path(path)
        .map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

    let mut entries = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

        // Parse fields
        let from: ProvinceId = record
            .get(0)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let to: ProvinceId = record
            .get(1)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let strait_type = record.get(2).unwrap_or("").trim().to_string();
        let through = record.get(3).and_then(|s| {
            if s.trim() == "-1" {
                None
            } else {
                s.trim().parse().ok()
            }
        });
        let start_x = record
            .get(4)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let start_y = record
            .get(5)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let stop_x = record
            .get(6)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let stop_y = record
            .get(7)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let adjacency_rule_name = record.get(8).map(|s| s.trim().to_string());
        let comment = record.get(9).map(|s| s.trim().to_string());

        entries.push(StraitEntry {
            from,
            to,
            strait_type,
            through,
            start_x,
            start_y,
            stop_x,
            stop_y,
            adjacency_rule_name,
            comment,
        });
    }

    Ok(entries)
}

/// Load province definitions from definition.csv.
///
/// Returns mapping of Color → ProvinceId.
pub fn load_definition_csv(path: &Path) -> Result<HashMap<Color, ProvinceId>, CacheError> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .comment(Some(b'#'))
        .from_path(path)
        .map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

    let mut color_map = HashMap::new();

    for result in reader.records() {
        let record = result.map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

        // Format: province;red;green;blue;x;x
        let province_id: ProvinceId = record
            .get(0)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let r: u8 = record
            .get(1)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let g: u8 = record
            .get(2)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        let b: u8 = record
            .get(3)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let color = Color { r, g, b };
        color_map.insert(color, province_id);
    }

    Ok(color_map)
}

impl CacheableResource for AdjacencyGraph {
    fn source_files(game_path: &Path) -> Vec<PathBuf> {
        vec![
            game_path.join("map").join("provinces.bmp"),
            game_path.join("map").join("definition.csv"),
            game_path.join("map").join("adjacencies.csv"),
        ]
    }

    fn generate(game_path: &Path) -> Result<Self, CacheError> {
        log::info!("Generating adjacency graph from game files...");

        // Load province color definitions
        let definition_path = game_path.join("map").join("definition.csv");
        let color_map = load_definition_csv(&definition_path)?;

        log::info!("Loaded {} province colors", color_map.len());

        // Generate adjacency from provinces.bmp
        let provinces_bmp_path = game_path.join("map").join("provinces.bmp");
        let mut graph = generate_adjacency_from_bmp(&provinces_bmp_path, &color_map)?;

        log::info!(
            "Generated base adjacency graph with {} provinces",
            graph.province_count()
        );

        // Add straits from adjacencies.csv
        let adjacencies_path = game_path.join("map").join("adjacencies.csv");
        if adjacencies_path.exists() {
            let straits = load_adjacencies_csv(&adjacencies_path)?;
            log::info!("Loaded {} strait entries", straits.len());

            for strait in straits {
                graph.add_adjacency(strait.from, strait.to);
            }
        }

        log::info!(
            "Final adjacency graph has {} provinces",
            graph.province_count()
        );

        Ok(graph)
    }
}

/// Generate adjacency graph from provinces.bmp.
///
/// Scans the BMP file and adds adjacencies for provinces whose pixels touch.
fn generate_adjacency_from_bmp(
    bmp_path: &Path,
    color_map: &HashMap<Color, ProvinceId>,
) -> Result<AdjacencyGraph, CacheError> {
    use image::ImageDecoder;
    use std::fs::File;
    use std::io::BufReader;

    log::info!("Loading provinces.bmp from {:?}", bmp_path);

    // Open BMP file
    let file = File::open(bmp_path)?;
    let reader = BufReader::new(file);

    // Decode BMP
    let decoder = image::codecs::bmp::BmpDecoder::new(reader)
        .map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

    let (width, height) = decoder.dimensions();
    log::info!("BMP dimensions: {}x{}", width, height);

    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| CacheError::Io(std::io::Error::other(e)))?
        .to_rgb8();

    log::info!("Scanning pixels for adjacencies...");

    let mut graph = AdjacencyGraph::new();
    let mut adjacency_set: HashSet<(ProvinceId, ProvinceId)> = HashSet::new();

    // Scan all pixels and check right/down neighbors
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            let color = Color {
                r: pixel[0],
                g: pixel[1],
                b: pixel[2],
            };

            if let Some(&province_id) = color_map.get(&color) {
                // Check right neighbor
                if x + 1 < width {
                    let neighbor_pixel = image.get_pixel(x + 1, y);
                    let neighbor_color = Color {
                        r: neighbor_pixel[0],
                        g: neighbor_pixel[1],
                        b: neighbor_pixel[2],
                    };

                    if let Some(&neighbor_id) = color_map.get(&neighbor_color)
                        && province_id != neighbor_id
                    {
                        let pair = if province_id < neighbor_id {
                            (province_id, neighbor_id)
                        } else {
                            (neighbor_id, province_id)
                        };
                        adjacency_set.insert(pair);
                    }
                }

                // Check down neighbor
                if y + 1 < height {
                    let neighbor_pixel = image.get_pixel(x, y + 1);
                    let neighbor_color = Color {
                        r: neighbor_pixel[0],
                        g: neighbor_pixel[1],
                        b: neighbor_pixel[2],
                    };

                    if let Some(&neighbor_id) = color_map.get(&neighbor_color)
                        && province_id != neighbor_id
                    {
                        let pair = if province_id < neighbor_id {
                            (province_id, neighbor_id)
                        } else {
                            (neighbor_id, province_id)
                        };
                        adjacency_set.insert(pair);
                    }
                }
            }
        }

        // Progress logging every 100 rows
        if y % 100 == 0 {
            log::debug!("Scanned {} / {} rows", y, height);
        }
    }

    log::info!("Found {} unique adjacencies", adjacency_set.len());

    // Add all adjacencies to graph
    for (p1, p2) in adjacency_set {
        graph.add_adjacency(p1, p2);
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjacency_graph_basic() {
        let mut graph = AdjacencyGraph::new();

        graph.add_adjacency(1, 2);
        graph.add_adjacency(2, 3);

        assert!(graph.are_adjacent(1, 2));
        assert!(graph.are_adjacent(2, 1)); // Bidirectional
        assert!(graph.are_adjacent(2, 3));
        assert!(!graph.are_adjacent(1, 3)); // Not adjacent

        let neighbors_2 = graph.neighbors(2);
        assert_eq!(neighbors_2.len(), 2);
        assert!(neighbors_2.contains(&1));
        assert!(neighbors_2.contains(&3));
    }

    #[test]
    fn test_color_equality() {
        let c1 = Color { r: 255, g: 0, b: 0 };
        let c2 = Color { r: 255, g: 0, b: 0 };
        let c3 = Color { r: 0, g: 255, b: 0 };

        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }
}
