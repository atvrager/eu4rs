//! Trade node definitions and topology loading from EU4 game data.
//!
//! Parses `common/tradenodes/*.txt` to build the complete trade network graph.
//! Includes cycle detection and topological sorting for value propagation.

use eu4txt::{DefaultEU4Txt, EU4Txt, EU4TxtAstItem, EU4TxtParseNode};
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::path::Path;
use std::sync::Mutex;

use rayon::prelude::*;

/// Unique identifier for a trade node (matches simulation's TradeNodeId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TradeNodeId(pub u16);

/// Static definition of a trade node (loaded from game data).
#[derive(Debug, Clone)]
pub struct TradeNodeDef {
    /// Internal name (e.g., "english_channel", "venice").
    pub name: String,

    /// Trade node ID for lookups.
    pub id: TradeNodeId,

    /// Representative province ID for map display.
    pub location: u32,

    /// Display color (RGB).
    pub color: [u8; 3],

    /// Whether this is an inland node (affects ship trade power).
    pub inland: bool,

    /// Outgoing trade routes (node IDs this node flows to).
    pub outgoing: Vec<TradeNodeId>,

    /// Outgoing route names (for matching before ID resolution).
    pub outgoing_names: Vec<String>,

    /// Member province IDs (provinces whose production flows here).
    pub members: Vec<u32>,
}

/// Complete trade network topology.
#[derive(Debug, Clone, Default)]
pub struct TradeNetwork {
    /// All trade node definitions, indexed by ID.
    pub nodes: Vec<TradeNodeDef>,

    /// Name to ID mapping for lookups.
    pub name_to_id: HashMap<String, TradeNodeId>,

    /// Province to trade node mapping.
    pub province_to_node: HashMap<u32, TradeNodeId>,

    /// Topological order (sources first, sinks last).
    /// Safe to iterate forward for value propagation.
    pub topological_order: Vec<TradeNodeId>,

    /// End nodes (no outgoing edges) - automatic collection points.
    pub end_nodes: Vec<TradeNodeId>,
}

/// Loads the complete trade network from `common/tradenodes/`.
///
/// Returns `Err` if:
/// - The directory doesn't exist
/// - Parsing fails
/// - A cycle is detected (EU4 trade should be a DAG)
pub fn load_trade_network(base_path: &Path) -> Result<TradeNetwork, Box<dyn Error + Send + Sync>> {
    let tradenodes_dir = base_path.join("common/tradenodes");

    if !tradenodes_dir.exists() {
        return Err(format!("Trade nodes directory not found: {:?}", tradenodes_dir).into());
    }

    // Collect all .txt files
    let entries: Vec<_> = std::fs::read_dir(tradenodes_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();

    // Parse all files in parallel, collect raw node data
    let raw_nodes: Mutex<Vec<RawNode>> = Mutex::new(Vec::new());

    entries.par_iter().for_each(|entry| {
        if let Ok(nodes) = parse_tradenodes_file(&entry.path()) {
            let mut lock = raw_nodes.lock().unwrap();
            lock.extend(nodes);
        }
    });

    let raw_nodes = raw_nodes.into_inner().unwrap();

    // Build the network
    build_network(raw_nodes)
}

/// Raw parsed node before ID resolution.
#[derive(Debug)]
struct RawNode {
    name: String,
    location: u32,
    color: [u8; 3],
    inland: bool,
    outgoing_names: Vec<String>,
    members: Vec<u32>,
}

/// Parse a single tradenodes file.
fn parse_tradenodes_file(path: &Path) -> Result<Vec<RawNode>, Box<dyn Error + Send + Sync>> {
    let tokens = DefaultEU4Txt::open_txt(path.to_str().unwrap()).map_err(|e| format!("{:?}", e))?;
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let ast = DefaultEU4Txt::parse(tokens).map_err(|e| format!("{:?}", e))?;
    let mut nodes = Vec::new();

    // Top level: node_name = { ... }
    if let EU4TxtAstItem::AssignmentList = ast.entry {
        for node in &ast.children {
            if let EU4TxtAstItem::Assignment = node.entry {
                let name_node = node.children.first().unwrap();
                let body_node = node.children.get(1).unwrap();

                if let EU4TxtAstItem::Identifier(name) = &name_node.entry
                    && let Some(raw) = parse_node_body(name.clone(), body_node)
                {
                    nodes.push(raw);
                }
            }
        }
    }

    Ok(nodes)
}

/// Parse the body of a trade node definition.
fn parse_node_body(name: String, body: &EU4TxtParseNode) -> Option<RawNode> {
    let mut location = 0u32;
    let mut color = [0u8; 3];
    let mut inland = false;
    let mut outgoing_names = Vec::new();
    let mut members = Vec::new();

    if let EU4TxtAstItem::AssignmentList = body.entry {
        for child in &body.children {
            if let EU4TxtAstItem::Assignment = child.entry {
                let key_node = child.children.first()?;
                let val_node = child.children.get(1)?;

                if let EU4TxtAstItem::Identifier(key) = &key_node.entry {
                    match key.as_str() {
                        "location" => {
                            location = parse_int_value(val_node).unwrap_or(0) as u32;
                        }
                        "color" => {
                            color = parse_color(val_node);
                        }
                        "inland" => {
                            inland = parse_yes_no(val_node);
                        }
                        "outgoing" => {
                            if let Some(target) = parse_outgoing(val_node) {
                                outgoing_names.push(target);
                            }
                        }
                        "members" => {
                            members = parse_members(val_node);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Some(RawNode {
        name,
        location,
        color,
        inland,
        outgoing_names,
        members,
    })
}

/// Parse an integer value node.
fn parse_int_value(node: &EU4TxtParseNode) -> Option<i32> {
    match &node.entry {
        EU4TxtAstItem::IntValue(i) => Some(*i),
        EU4TxtAstItem::Identifier(s) => s.parse().ok(),
        EU4TxtAstItem::FloatValue(f) => Some(*f as i32),
        _ => None,
    }
}

/// Parse color block: { R G B }
fn parse_color(node: &EU4TxtParseNode) -> [u8; 3] {
    let mut color = [0u8; 3];
    if let EU4TxtAstItem::AssignmentList = node.entry {
        for (i, child) in node.children.iter().take(3).enumerate() {
            if let Some(v) = parse_int_value(child) {
                color[i] = v.clamp(0, 255) as u8;
            }
        }
    }
    color
}

/// Parse yes/no value.
fn parse_yes_no(node: &EU4TxtParseNode) -> bool {
    if let EU4TxtAstItem::Identifier(s) = &node.entry {
        return s == "yes";
    }
    false
}

/// Parse outgoing block to extract target node name.
fn parse_outgoing(node: &EU4TxtParseNode) -> Option<String> {
    if let EU4TxtAstItem::AssignmentList = node.entry {
        for child in &node.children {
            if let EU4TxtAstItem::Assignment = child.entry {
                let key = child.children.first()?;
                let val = child.children.get(1)?;

                if let EU4TxtAstItem::Identifier(k) = &key.entry
                    && k == "name"
                {
                    if let EU4TxtAstItem::StringValue(s) = &val.entry {
                        return Some(s.clone());
                    } else if let EU4TxtAstItem::Identifier(s) = &val.entry {
                        return Some(s.clone());
                    }
                }
            }
        }
    }
    None
}

/// Parse members block: { 1 2 3 ... }
fn parse_members(node: &EU4TxtParseNode) -> Vec<u32> {
    let mut members = Vec::new();
    if let EU4TxtAstItem::AssignmentList = node.entry {
        for child in &node.children {
            if let Some(v) = parse_int_value(child) {
                members.push(v as u32);
            }
        }
    }
    members
}

/// Build the complete network from raw parsed nodes.
fn build_network(raw_nodes: Vec<RawNode>) -> Result<TradeNetwork, Box<dyn Error + Send + Sync>> {
    // First pass: assign IDs and build name→id mapping
    let mut name_to_id: HashMap<String, TradeNodeId> = HashMap::new();
    for (i, raw) in raw_nodes.iter().enumerate() {
        let id = TradeNodeId(i as u16);
        name_to_id.insert(raw.name.clone(), id);
    }

    // Second pass: convert raw nodes to TradeNodeDef with resolved IDs
    let mut nodes: Vec<TradeNodeDef> = Vec::with_capacity(raw_nodes.len());
    let mut province_to_node: HashMap<u32, TradeNodeId> = HashMap::new();

    for (i, raw) in raw_nodes.into_iter().enumerate() {
        let id = TradeNodeId(i as u16);

        // Resolve outgoing names to IDs
        let outgoing: Vec<TradeNodeId> = raw
            .outgoing_names
            .iter()
            .filter_map(|name| name_to_id.get(name).copied())
            .collect();

        // Build province mapping
        for &prov in &raw.members {
            province_to_node.insert(prov, id);
        }

        nodes.push(TradeNodeDef {
            name: raw.name,
            id,
            location: raw.location,
            color: raw.color,
            inland: raw.inland,
            outgoing,
            outgoing_names: raw.outgoing_names,
            members: raw.members,
        });
    }

    // Detect cycles and compute topological order
    let (topological_order, has_cycle) = topological_sort(&nodes);
    if has_cycle {
        return Err("Trade network contains a cycle! This is invalid for EU4.".into());
    }

    // Find end nodes (no outgoing edges)
    let end_nodes: Vec<TradeNodeId> = nodes
        .iter()
        .filter(|n| n.outgoing.is_empty())
        .map(|n| n.id)
        .collect();

    log::info!(
        "Loaded {} trade nodes, {} end nodes, {} provinces mapped",
        nodes.len(),
        end_nodes.len(),
        province_to_node.len()
    );

    Ok(TradeNetwork {
        nodes,
        name_to_id,
        province_to_node,
        topological_order,
        end_nodes,
    })
}

/// Compute topological order using Kahn's algorithm.
/// Returns (order, has_cycle).
fn topological_sort(nodes: &[TradeNodeDef]) -> (Vec<TradeNodeId>, bool) {
    let n = nodes.len();
    if n == 0 {
        return (Vec::new(), false);
    }

    // Compute in-degree for each node
    let mut in_degree: Vec<usize> = vec![0; n];
    for node in nodes {
        for &target in &node.outgoing {
            in_degree[target.0 as usize] += 1;
        }
    }

    // Queue of nodes with no incoming edges
    let mut queue: VecDeque<TradeNodeId> = VecDeque::new();
    for (i, &deg) in in_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(TradeNodeId(i as u16));
        }
    }

    let mut order = Vec::with_capacity(n);

    while let Some(node_id) = queue.pop_front() {
        order.push(node_id);

        // Reduce in-degree of neighbors
        for &target in &nodes[node_id.0 as usize].outgoing {
            in_degree[target.0 as usize] -= 1;
            if in_degree[target.0 as usize] == 0 {
                queue.push_back(target);
            }
        }
    }

    // If we didn't visit all nodes, there's a cycle
    let has_cycle = order.len() != n;

    (order, has_cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::detect_game_path;
    use std::collections::HashSet;

    #[test]
    fn test_load_trade_network() {
        let Some(game_path) = detect_game_path() else {
            eprintln!("Skipping test: EU4 not found");
            return;
        };

        let network = load_trade_network(&game_path).expect("Failed to load trade network");

        // Basic sanity checks
        assert!(!network.nodes.is_empty(), "Should load trade nodes");
        assert!(
            network.nodes.len() >= 80,
            "EU4 has ~80 trade nodes, got {}",
            network.nodes.len()
        );

        // Check for known trade nodes (names vary slightly between EU4 versions)
        let has_english_channel = network.name_to_id.contains_key("english_channel");
        let has_venice = network.name_to_id.contains_key("venice");
        assert!(
            has_english_channel || has_venice,
            "Should have at least one major end node (English Channel or Venice). Found nodes: {:?}",
            network.name_to_id.keys().take(10).collect::<Vec<_>>()
        );

        // Check end nodes exist
        assert!(!network.end_nodes.is_empty(), "Should have end nodes");

        let end_node_names: HashSet<&str> = network
            .end_nodes
            .iter()
            .map(|id| network.nodes[id.0 as usize].name.as_str())
            .collect();

        // End nodes should include major trade destinations
        println!(
            "Found {} end nodes: {:?}",
            end_node_names.len(),
            end_node_names
        );

        // Venice and English Channel are historically important end nodes
        let has_major_end_node =
            end_node_names.contains("venice") || end_node_names.contains("english_channel");

        assert!(
            has_major_end_node,
            "Should have Venice or English Channel as end nodes, got: {:?}",
            end_node_names
        );

        // Check topological order is valid
        assert_eq!(
            network.topological_order.len(),
            network.nodes.len(),
            "Topological order should include all nodes"
        );

        // Verify topological order property: for each edge (u → v), u comes before v
        let node_position: HashMap<TradeNodeId, usize> = network
            .topological_order
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        for node in &network.nodes {
            for &target in &node.outgoing {
                let source_pos = node_position[&node.id];
                let target_pos = node_position[&target];
                assert!(
                    source_pos < target_pos,
                    "Topological order violated: {} (pos {}) -> {} (pos {})",
                    node.name,
                    source_pos,
                    network.nodes[target.0 as usize].name,
                    target_pos
                );
            }
        }

        // Check province mapping
        assert!(
            !network.province_to_node.is_empty(),
            "Should have province mappings"
        );

        // London (236) should be in English Channel
        if let Some(&node_id) = network.province_to_node.get(&236) {
            let node = &network.nodes[node_id.0 as usize];
            assert_eq!(
                node.name, "english_channel",
                "London should be in English Channel"
            );
        }

        println!(
            "Loaded {} trade nodes, {} end nodes",
            network.nodes.len(),
            network.end_nodes.len()
        );
        println!("End nodes: {:?}", end_node_names);
    }

    #[test]
    fn test_topological_sort_simple() {
        // Create a simple DAG: A → B → C
        let nodes = vec![
            TradeNodeDef {
                name: "A".to_string(),
                id: TradeNodeId(0),
                location: 1,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(1)],
                outgoing_names: vec!["B".to_string()],
                members: vec![1],
            },
            TradeNodeDef {
                name: "B".to_string(),
                id: TradeNodeId(1),
                location: 2,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(2)],
                outgoing_names: vec!["C".to_string()],
                members: vec![2],
            },
            TradeNodeDef {
                name: "C".to_string(),
                id: TradeNodeId(2),
                location: 3,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![],
                outgoing_names: vec![],
                members: vec![3],
            },
        ];

        let (order, has_cycle) = topological_sort(&nodes);

        assert!(!has_cycle, "Simple chain should not have cycle");
        assert_eq!(order.len(), 3);

        // A should come before B, B before C
        let positions: HashMap<u16, usize> =
            order.iter().enumerate().map(|(i, id)| (id.0, i)).collect();

        assert!(positions[&0] < positions[&1], "A should come before B");
        assert!(positions[&1] < positions[&2], "B should come before C");
    }

    #[test]
    fn test_topological_sort_cycle_detection() {
        // Create a cycle: A → B → A
        let nodes = vec![
            TradeNodeDef {
                name: "A".to_string(),
                id: TradeNodeId(0),
                location: 1,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(1)],
                outgoing_names: vec!["B".to_string()],
                members: vec![1],
            },
            TradeNodeDef {
                name: "B".to_string(),
                id: TradeNodeId(1),
                location: 2,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(0)], // Cycle back to A
                outgoing_names: vec!["A".to_string()],
                members: vec![2],
            },
        ];

        let (order, has_cycle) = topological_sort(&nodes);

        assert!(has_cycle, "Should detect cycle");
        assert!(order.len() < 2, "Should not complete ordering with cycle");
    }

    #[test]
    fn test_topological_sort_diamond() {
        // Diamond: A → B, A → C, B → D, C → D
        let nodes = vec![
            TradeNodeDef {
                name: "A".to_string(),
                id: TradeNodeId(0),
                location: 1,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(1), TradeNodeId(2)],
                outgoing_names: vec!["B".to_string(), "C".to_string()],
                members: vec![1],
            },
            TradeNodeDef {
                name: "B".to_string(),
                id: TradeNodeId(1),
                location: 2,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(3)],
                outgoing_names: vec!["D".to_string()],
                members: vec![2],
            },
            TradeNodeDef {
                name: "C".to_string(),
                id: TradeNodeId(2),
                location: 3,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![TradeNodeId(3)],
                outgoing_names: vec!["D".to_string()],
                members: vec![3],
            },
            TradeNodeDef {
                name: "D".to_string(),
                id: TradeNodeId(3),
                location: 4,
                color: [0, 0, 0],
                inland: false,
                outgoing: vec![],
                outgoing_names: vec![],
                members: vec![4],
            },
        ];

        let (order, has_cycle) = topological_sort(&nodes);

        assert!(!has_cycle, "Diamond should not have cycle");
        assert_eq!(order.len(), 4);

        let positions: HashMap<u16, usize> =
            order.iter().enumerate().map(|(i, id)| (id.0, i)).collect();

        // A before B, C, and D
        assert!(positions[&0] < positions[&1]);
        assert!(positions[&0] < positions[&2]);
        assert!(positions[&0] < positions[&3]);
        // B and C before D
        assert!(positions[&1] < positions[&3]);
        assert!(positions[&2] < positions[&3]);
    }
}
