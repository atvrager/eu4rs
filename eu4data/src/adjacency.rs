use crate::cache::{CacheError, CacheableResource};
use game_pathfinding::Graph;
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
    /// Set of directed river crossings (from, to) that apply combat penalty
    #[serde(default)]
    pub river_crossings: HashSet<(ProvinceId, ProvinceId)>,
}

impl AdjacencyGraph {
    /// Create a new empty adjacency graph.
    pub fn new() -> Self {
        Self {
            adjacencies: HashMap::new(),
            river_crossings: HashSet::new(),
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

    /// Check if moving from `from` to `to` crosses a river.
    ///
    /// River crossings apply a -1 dice penalty to the attacker in combat.
    pub fn is_river_crossing(&self, from: ProvinceId, to: ProvinceId) -> bool {
        self.river_crossings.contains(&(from, to))
    }

    /// Get total number of provinces in the graph.
    pub fn province_count(&self) -> usize {
        self.adjacencies.len()
    }

    /// Find a path from start to end province using BFS.
    ///
    /// Returns the full path including the destination, but excluding the start.
    /// Returns None if no path exists.
    pub fn find_path(&self, start: ProvinceId, end: ProvinceId) -> Option<Vec<ProvinceId>> {
        use std::collections::VecDeque;

        // Early exit if start equals end
        if start == end {
            return Some(Vec::new());
        }

        // BFS to find shortest path
        let mut queue: VecDeque<ProvinceId> = VecDeque::new();
        let mut visited: HashSet<ProvinceId> = HashSet::new();
        let mut parent: HashMap<ProvinceId, ProvinceId> = HashMap::new();

        queue.push_back(start);
        visited.insert(start);

        while let Some(current) = queue.pop_front() {
            // Found the destination
            if current == end {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = end;

                while node != start {
                    path.push(node);
                    node = *parent.get(&node)?;
                }

                path.reverse();
                return Some(path);
            }

            // Explore neighbors
            if let Some(neighbors) = self.adjacencies.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        parent.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // No path found
        None
    }
}

impl Default for AdjacencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for calculating movement costs between provinces.
/// Implement this for your game state or map mode context.
pub trait CostCalculator {
    fn calculate_cost(&self, from: ProvinceId, to: ProvinceId) -> u32;
    fn calculate_heuristic(&self, from: ProvinceId, to: ProvinceId) -> u32;
}

/// Implement the generic Graph trait for AdjacencyGraph.
/// This allows us to use game_pathfinding algorithms on our map.
impl<C> Graph<ProvinceId, C> for AdjacencyGraph
where
    C: CostCalculator,
{
    fn neighbors(&self, node: ProvinceId, _context: &C) -> Vec<ProvinceId> {
        self.neighbors(node)
    }

    fn cost(&self, from: ProvinceId, to: ProvinceId, context: &C) -> u32 {
        context.calculate_cost(from, to)
    }

    fn heuristic(&self, from: ProvinceId, target: ProvinceId, context: &C) -> u32 {
        context.calculate_heuristic(from, target)
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
    let content_bytes = std::fs::read(path).map_err(CacheError::Io)?;
    let (content_str, _, _) = encoding_rs::WINDOWS_1252.decode(&content_bytes);

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .comment(Some(b'#'))
        .flexible(true)
        .from_reader(content_str.as_bytes());

    let mut entries = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| CacheError::Io(std::io::Error::other(e)))?;

        // Parse fields
        let from_val = record
            .get(0)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(0);
        let to_val = record
            .get(1)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(0);

        if from_val < 0 || to_val < 0 {
            continue;
        }

        let from = from_val as ProvinceId;
        let to = to_val as ProvinceId;
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

        // Optional fields
        let (rule_name, comment) = if record.len() >= 10 {
            (
                record.get(8).map(|s| s.trim().to_string()),
                record.get(9).map(|s| s.trim().to_string()),
            )
        } else {
            (None, record.get(8).map(|s| s.trim().to_string()))
        };

        entries.push(StraitEntry {
            from,
            to,
            strait_type,
            through,
            start_x,
            start_y,
            stop_x,
            stop_y,
            adjacency_rule_name: rule_name,
            comment,
        });
    }

    Ok(entries)
}

/// Load province definitions from definition.csv.
///
/// Returns mapping of Color → ProvinceId.
pub fn load_definition_csv(path: &Path) -> Result<HashMap<Color, ProvinceId>, CacheError> {
    let content_bytes = std::fs::read(path).map_err(CacheError::Io)?;
    let (content_str, _, _) = encoding_rs::WINDOWS_1252.decode(&content_bytes);

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .comment(Some(b'#'))
        .flexible(true)
        .from_reader(content_str.as_bytes());

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

        // Add special adjacencies from adjacencies.csv (straits, rivers, etc.)
        let adjacencies_path = game_path.join("map").join("adjacencies.csv");
        if adjacencies_path.exists() {
            let entries = load_adjacencies_csv(&adjacencies_path)?;
            log::info!("Loaded {} adjacency entries", entries.len());

            let mut river_count = 0;
            for entry in entries {
                // Add adjacency for all types (straits, rivers, land, etc.)
                graph.add_adjacency(entry.from, entry.to);

                // Track river crossings for combat penalties
                if entry.strait_type == "river" {
                    graph.river_crossings.insert((entry.from, entry.to));
                    graph.river_crossings.insert((entry.to, entry.from));
                    river_count += 1;
                }
            }

            if river_count > 0 {
                log::info!("Detected {} river crossings", river_count);
            }
        }

        log::info!(
            "Final adjacency graph has {} provinces",
            graph.province_count()
        );

        Ok(graph)
    }
}

/// Load the adjacency graph from cache or generate it.
pub fn load_adjacency_graph(
    game_path: &Path,
    mode: crate::cache::CacheValidationMode,
) -> Result<AdjacencyGraph, crate::cache::CacheError> {
    crate::cache::load_or_generate("adjacency_graph", game_path, false, mode)
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

    #[test]
    fn test_find_path_adjacent() {
        let mut graph = AdjacencyGraph::new();
        graph.add_adjacency(1, 2);

        let path = graph.find_path(1, 2);
        assert_eq!(path, Some(vec![2]));
    }

    #[test]
    fn test_find_path_multi_province() {
        let mut graph = AdjacencyGraph::new();
        // Create path: 1 -> 2 -> 3 -> 4
        graph.add_adjacency(1, 2);
        graph.add_adjacency(2, 3);
        graph.add_adjacency(3, 4);

        let path = graph.find_path(1, 4);
        assert_eq!(path, Some(vec![2, 3, 4]));
    }

    #[test]
    fn test_find_path_same_province() {
        let mut graph = AdjacencyGraph::new();
        graph.add_adjacency(1, 2);

        let path = graph.find_path(1, 1);
        assert_eq!(path, Some(vec![]));
    }

    #[test]
    fn test_find_path_no_connection() {
        let mut graph = AdjacencyGraph::new();
        graph.add_adjacency(1, 2);
        graph.add_adjacency(3, 4);

        let path = graph.find_path(1, 4);
        assert_eq!(path, None);
    }

    #[test]
    fn test_find_path_multiple_routes_finds_shortest() {
        let mut graph = AdjacencyGraph::new();
        // Create a graph with multiple paths:
        // 1 -> 2 -> 5
        // 1 -> 3 -> 4 -> 5
        graph.add_adjacency(1, 2);
        graph.add_adjacency(2, 5);
        graph.add_adjacency(1, 3);
        graph.add_adjacency(3, 4);
        graph.add_adjacency(4, 5);

        let path = graph.find_path(1, 5);
        // BFS should find shortest path: 1 -> 2 -> 5
        assert_eq!(path, Some(vec![2, 5]));
    }

    #[test]
    fn test_encoding_preservation() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let def_path = dir.path().join("definition.csv");

        // 0xE5 is 'å' in WINDOWS-1252
        let raw_bytes = b"province;red;green;blue;x;x\n2;0;36;128;Sk\xE5ne;x\n";

        let mut f = File::create(&def_path).unwrap();
        f.write_all(raw_bytes).unwrap();

        let color_map = load_definition_csv(&def_path).unwrap();

        // Find the province ID for the color 0, 36, 128
        let color = Color {
            r: 0,
            g: 36,
            b: 128,
        };
        assert_eq!(color_map.get(&color), Some(&2));

        let adj_path = dir.path().join("adjacencies.csv");
        // "Skåne-Sjaelland" in WINDOWS-1252
        let adj_bytes = b"From;To;Type;Through;start_x;start_y;stop_x;stop_y;Comment\n6;12;sea;1258;3008;1633;3000;1630;Sk\xE5ne-Sjaelland\n-1;-1;;;;;;;\n";

        let mut f2 = File::create(&adj_path).unwrap();
        f2.write_all(adj_bytes).unwrap();

        let straits = load_adjacencies_csv(&adj_path).unwrap();
        assert_eq!(straits.len(), 1);
        assert_eq!(straits[0].comment.as_deref(), Some("Sk\u{e5}ne-Sjaelland"));
    }

    // Mock cost calculator for testing A* integration
    struct MockCost;
    impl CostCalculator for MockCost {
        fn calculate_cost(&self, _from: ProvinceId, _to: ProvinceId) -> u32 {
            10 // High movement cost
        }
        fn calculate_heuristic(&self, _from: ProvinceId, _to: ProvinceId) -> u32 {
            0 // Dijkstra
        }
    }

    #[test]
    fn test_astar_integration() {
        let mut graph = AdjacencyGraph::new();
        graph.add_adjacency(1, 2);
        graph.add_adjacency(2, 3);

        let ctx = MockCost;

        use game_pathfinding::AStar;
        let result = AStar::find_path(&graph, 1, 3, &ctx);

        assert!(result.is_some());
        let (path, cost) = result.unwrap();
        assert_eq!(path, vec![1, 2, 3]);
        assert_eq!(cost, 20); // 10 + 10
    }
}
