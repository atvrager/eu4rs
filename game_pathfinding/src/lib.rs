use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hash;

/// A trait for graphs that can be searched.
///
/// `Node`: The type of node identifiers (e.g., ProvinceId).
/// `Ctx`: A context object passed to cost calculations (e.g., WorldState, MapMode).
pub trait Graph<Node, Ctx> {
    /// Return an iterator over the neighbors of a node.
    fn neighbors(&self, node: Node, context: &Ctx) -> Vec<Node>;

    /// Calculate the cost to move from `from` to `to`.
    /// This allows dynamic weighting based on the provided context.
    fn cost(&self, from: Node, to: Node, context: &Ctx) -> u32;

    /// Calculate the estimated cost (heuristic) from `from` to `target`.
    /// For A*, this must be admissible (never overestimate).
    fn heuristic(&self, from: Node, target: Node, context: &Ctx) -> u32;
}

/// A generic A* pathfinder.
pub struct AStar;

impl AStar {
    /// Find the shortest path from `start` to `goal`.
    pub fn find_path<Node, Ctx, G>(
        graph: &G,
        start: Node,
        goal: Node,
        context: &Ctx,
    ) -> Option<(Vec<Node>, u32)>
    where
        Node: Copy + Eq + Hash + std::fmt::Debug,
        G: Graph<Node, Ctx>,
    {
        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<Node, Node> = HashMap::new();
        let mut g_score: HashMap<Node, u32> = HashMap::new();
        let mut closed_set: HashSet<Node> = HashSet::new();

        g_score.insert(start, 0);
        open_set.push(State {
            node: start,
            cost: 0,
            priority: graph.heuristic(start, goal, context),
        });

        while let Some(State { node: current, .. }) = open_set.pop() {
            // Skip if already processed with a better path
            if !closed_set.insert(current) {
                continue;
            }

            if current == goal {
                // Reconstruct path
                let mut path = vec![current];
                let mut curr = current;
                while let Some(&prev) = came_from.get(&curr) {
                    path.push(prev);
                    curr = prev;
                }
                path.reverse();
                return Some((path, g_score[&goal]));
            }

            let current_g = g_score[&current];

            for neighbor in graph.neighbors(current, context) {
                // Skip already-processed nodes
                if closed_set.contains(&neighbor) {
                    continue;
                }

                let tentative_g = current_g + graph.cost(current, neighbor, context);

                if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    open_set.push(State {
                        node: neighbor,
                        cost: tentative_g,
                        priority: tentative_g + graph.heuristic(neighbor, goal, context),
                    });
                }
            }
        }

        None
    }
}

/// Helper struct for the priority queue.
#[derive(Copy, Clone, Eq, PartialEq)]
struct State<Node> {
    node: Node,
    cost: u32,     // Actual cost from start (g_score)
    priority: u32, // Estimated total cost (f_score = g + h)
}

// The priority queue depends on `Ord`.
// Explicitly implement the trait so the queue becomes a min-heap.
impl<Node: Eq> Ord for State<Node> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Notice that we flip the ordering on costs.
        // In case of a tie we compare positions - this step is necessary
        // to make implementations of `PartialEq` and `Ord` consistent.
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.cost.cmp(&self.cost))
    }
}

// `PartialOrd` needs to be implemented as well.
impl<Node: Eq> PartialOrd for State<Node> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple grid graph for testing
    // 0 1 2
    // 3 4 5
    // 6 7 8
    struct GridGraph;

    impl Graph<u32, ()> for GridGraph {
        fn neighbors(&self, node: u32, _context: &()) -> Vec<u32> {
            let mut n = Vec::new();
            let x = node % 3;
            let y = node / 3;

            if x > 0 {
                n.push(node - 1);
            } // Left
            if x < 2 {
                n.push(node + 1);
            } // Right
            if y > 0 {
                n.push(node - 3);
            } // Up
            if y < 2 {
                n.push(node + 3);
            } // Down
            n
        }

        fn cost(&self, _from: u32, _to: u32, _context: &()) -> u32 {
            1 // Uniform cost
        }

        fn heuristic(&self, from: u32, target: u32, _context: &()) -> u32 {
            // Manhattan distance
            let x1 = (from % 3) as i32;
            let y1 = (from / 3) as i32;
            let x2 = (target % 3) as i32;
            let y2 = (target / 3) as i32;
            ((x1 - x2).abs() + (y1 - y2).abs()) as u32
        }
    }

    #[test]
    fn test_grid_pathfinding() {
        let graph = GridGraph;
        let start = 0; // Top-left
        let goal = 8; // Bottom-right

        let result = AStar::find_path(&graph, start, goal, &());
        assert!(result.is_some());

        let (path, cost) = result.unwrap();
        // Shortest path is 4 steps (e.g. 0->1->2->5->8 or 0->3->6->7->8)
        assert_eq!(cost, 4);
        assert_eq!(path.first(), Some(&0));
        assert_eq!(path.last(), Some(&8));
        assert_eq!(path.len(), 5); // Includes start node
    }

    struct WeightedGraph; // 0 -> 1 (cost 10), 0 -> 2 (cost 1), 2 -> 1 (cost 1)

    impl Graph<u32, ()> for WeightedGraph {
        fn neighbors(&self, node: u32, _context: &()) -> Vec<u32> {
            match node {
                0 => vec![1, 2],
                2 => vec![1],
                _ => vec![],
            }
        }

        fn cost(&self, from: u32, to: u32, _context: &()) -> u32 {
            match (from, to) {
                (0, 1) => 10,
                (0, 2) => 1,
                (2, 1) => 1,
                _ => 1,
            }
        }

        fn heuristic(&self, _from: u32, _target: u32, _context: &()) -> u32 {
            0
        } // Dijkstra
    }

    #[test]
    fn test_weighted_pathfinding() {
        let graph = WeightedGraph;
        // Should go 0 -> 2 -> 1 (cost 2) instead of 0 -> 1 (cost 10)
        let (path, cost) = AStar::find_path(&graph, 0, 1, &()).unwrap();
        assert_eq!(cost, 2);
        assert_eq!(path, vec![0, 2, 1]);
    }

    // Graph with many paths to test closed set prevents duplicate processing
    // Diamond shape: 0 -> {1, 2} -> 3
    struct DiamondGraph;

    impl Graph<u32, ()> for DiamondGraph {
        fn neighbors(&self, node: u32, _context: &()) -> Vec<u32> {
            match node {
                0 => vec![1, 2],
                1 => vec![3],
                2 => vec![3],
                _ => vec![],
            }
        }

        fn cost(&self, _from: u32, _to: u32, _context: &()) -> u32 {
            1
        }

        fn heuristic(&self, _from: u32, _target: u32, _context: &()) -> u32 {
            0
        }
    }

    #[test]
    fn test_no_duplicate_processing() {
        let graph = DiamondGraph;
        // Both paths 0->1->3 and 0->2->3 reach node 3
        // Without closed set, node 3 could be processed twice
        let (path, cost) = AStar::find_path(&graph, 0, 3, &()).unwrap();
        assert_eq!(cost, 2);
        assert!(path == vec![0, 1, 3] || path == vec![0, 2, 3]);
        assert_eq!(path.len(), 3);
    }
}
