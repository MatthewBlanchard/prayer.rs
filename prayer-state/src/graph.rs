//! All-pairs routing over the system jump graph.
//!
//! The galaxy is small (`get_map` returns every system with its connections
//! in one response), so instead of searching per query we run one BFS from
//! every system and answer all routing queries from the resulting table:
//! a hop-distance matrix plus a first-hop matrix that shortest paths are
//! reconstructed from by chaining. When stronghold avoidance is configured,
//! the first-hop matrix is built from weighted route costs while the naked
//! hop matrix remains available for true jump distances.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

/// Matrix sentinel for "no route" / "no hop".
const UNREACHABLE: u32 = u32::MAX;

/// All-pairs hop distances and first hops over the jump graph.
///
/// Build cost is `O(V * (V + E))` and memory `O(V^2)` — both trivial for a
/// galaxy of hundreds of systems. Neighbor order is sorted during the
/// build, so equal-length routes resolve deterministically.
#[derive(Debug, Default)]
pub struct RouteTable {
    /// Sorted system ids; defines row/column order for the matrices.
    nodes: Vec<String>,
    /// System id -> index in `nodes`.
    index: HashMap<String, u32>,
    /// Row-major hop counts from each source row.
    dist: Vec<u32>,
    /// Row-major weighted travel costs from each source row.
    cost: Vec<u32>,
    /// Row-major first hop on a shortest route from each source row.
    first_hop: Vec<u32>,
    /// Row-major first hop on an unweighted shortest route.
    naked_first_hop: Vec<u32>,
}

impl RouteTable {
    /// Run BFS from every system in `connections` (treated as a directed
    /// adjacency list, matching how the map stores it).
    pub fn build(connections: &HashMap<String, Vec<String>>) -> Self {
        Self::build_with_penalties(connections, &HashSet::new())
    }

    /// Build routes where entering a penalized system costs two jumps.
    pub fn build_with_penalties(
        connections: &HashMap<String, Vec<String>>,
        penalized_systems: &HashSet<String>,
    ) -> Self {
        let mut nodes: Vec<&str> = connections
            .keys()
            .map(String::as_str)
            .chain(connections.values().flatten().map(String::as_str))
            .collect();
        nodes.sort_unstable();
        nodes.dedup();
        let nodes: Vec<String> = nodes.into_iter().map(str::to_string).collect();
        let index: HashMap<String, u32> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.clone(), i as u32))
            .collect();

        let n = nodes.len();
        let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (system, neighbors) in connections {
            let mut targets: Vec<u32> = neighbors
                .iter()
                .filter_map(|neighbor| index.get(neighbor).copied())
                .collect();
            targets.sort_unstable();
            targets.dedup();
            adjacency[index[system] as usize] = targets;
        }

        let mut dist = vec![UNREACHABLE; n * n];
        let mut cost = vec![UNREACHABLE; n * n];
        let mut first_hop = vec![UNREACHABLE; n * n];
        let mut naked_first_hop = vec![UNREACHABLE; n * n];
        let mut queue = VecDeque::new();
        let mut heap = BinaryHeap::new();
        for source in 0..n {
            let row = source * n;
            dist[row + source] = 0;
            queue.clear();
            queue.push_back(source as u32);
            while let Some(current) = queue.pop_front() {
                for &neighbor in &adjacency[current as usize] {
                    if dist[row + neighbor as usize] != UNREACHABLE {
                        continue;
                    }
                    dist[row + neighbor as usize] = dist[row + current as usize] + 1;
                    naked_first_hop[row + neighbor as usize] = if current as usize == source {
                        neighbor
                    } else {
                        naked_first_hop[row + current as usize]
                    };
                    queue.push_back(neighbor);
                }
            }

            cost[row + source] = 0;
            heap.clear();
            heap.push(Reverse((0_u32, source as u32)));
            while let Some(Reverse((current_cost, current))) = heap.pop() {
                if current_cost != cost[row + current as usize] {
                    continue;
                }
                for &neighbor in &adjacency[current as usize] {
                    let hop_cost = if penalized_systems.contains(&nodes[neighbor as usize]) {
                        2
                    } else {
                        1
                    };
                    let next_cost = current_cost.saturating_add(hop_cost);
                    let cell = row + neighbor as usize;
                    if next_cost >= cost[cell] {
                        continue;
                    }
                    cost[cell] = next_cost;
                    first_hop[cell] = if current as usize == source {
                        neighbor
                    } else {
                        first_hop[row + current as usize]
                    };
                    heap.push(Reverse((next_cost, neighbor)));
                }
            }
        }

        Self {
            nodes,
            index,
            dist,
            cost,
            first_hop,
            naked_first_hop,
        }
    }

    /// Return all systems within `max_hops` naked jumps of the given systems.
    pub fn systems_within_hops(
        connections: &HashMap<String, Vec<String>>,
        origins: &HashSet<String>,
        max_hops: usize,
    ) -> HashSet<String> {
        if origins.is_empty() {
            return HashSet::new();
        }

        let naked = Self::build(connections);
        naked
            .nodes
            .iter()
            .filter(|system| {
                origins.iter().any(|origin| {
                    naked
                        .hop_distance(origin.as_str(), system.as_str())
                        .is_some_and(|distance| distance <= max_hops)
                })
            })
            .cloned()
            .collect()
    }

    fn pair(&self, start: &str, target: &str) -> Option<(usize, usize)> {
        Some((
            *self.index.get(start)? as usize,
            *self.index.get(target)? as usize,
        ))
    }

    /// Hop count between systems; `Some(0)` when `start` equals `target`.
    pub fn hop_distance(&self, start: &str, target: &str) -> Option<usize> {
        if start == target {
            return Some(0);
        }
        let (start, target) = self.pair(start, target)?;
        let dist = self.dist[start * self.nodes.len() + target];
        (dist != UNREACHABLE).then_some(dist as usize)
    }

    /// Weighted route cost between systems; entering penalized systems counts
    /// as two jumps. `Some(0)` when `start` equals `target`.
    pub fn path_cost(&self, start: &str, target: &str) -> Option<usize> {
        if start == target {
            return Some(0);
        }
        let (start, target) = self.pair(start, target)?;
        let cost = self.cost[start * self.nodes.len() + target];
        (cost != UNREACHABLE).then_some(cost as usize)
    }

    /// First hop on a shortest route from `start` toward `target`.
    pub fn next_hop_toward(&self, start: &str, target: &str) -> Option<String> {
        if start == target {
            return None;
        }
        let (start, target) = self.pair(start, target)?;
        let hop = self.first_hop[start * self.nodes.len() + target];
        (hop != UNREACHABLE).then(|| self.nodes[hop as usize].clone())
    }

    /// Shortest hop sequence from `start` to `target`, excluding `start`
    /// and including `target`. Reconstructed by chaining first hops: every
    /// hop lands on a node whose own shortest route continues the path.
    pub fn path_hops(&self, start: &str, target: &str) -> Option<Vec<String>> {
        self.path_hops_with_table(start, target, &self.cost, &self.first_hop)
    }

    /// Unweighted shortest hop sequence from `start` to `target`.
    pub fn naked_path_hops(&self, start: &str, target: &str) -> Option<Vec<String>> {
        self.path_hops_with_table(start, target, &self.dist, &self.naked_first_hop)
    }

    fn path_hops_with_table(
        &self,
        start: &str,
        target: &str,
        reachable: &[u32],
        first_hop: &[u32],
    ) -> Option<Vec<String>> {
        if start == target {
            return Some(Vec::new());
        }
        let (mut current, target) = self.pair(start, target)?;
        let n = self.nodes.len();
        if reachable[current * n + target] == UNREACHABLE {
            return None;
        }
        let mut path = Vec::new();
        while current != target {
            let hop = first_hop[current * n + target] as usize;
            path.push(self.nodes[hop].clone());
            current = hop;
        }
        Some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_graph() -> HashMap<String, Vec<String>> {
        HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            (
                "alpha".to_string(),
                vec!["sol".to_string(), "beta".to_string()],
            ),
            ("beta".to_string(), vec!["alpha".to_string()]),
        ])
    }

    #[test]
    fn path_hops_returns_hop_sequence() {
        let table = RouteTable::build(&line_graph());
        let hops = table.path_hops("sol", "beta").expect("path");
        assert_eq!(hops, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn next_hop_toward_returns_first_hop() {
        let table = RouteTable::build(&line_graph());
        assert_eq!(
            table.next_hop_toward("sol", "beta"),
            Some("alpha".to_string())
        );
        assert_eq!(table.next_hop_toward("sol", "sol"), None);
    }

    #[test]
    fn hop_distance_counts_edges() {
        let table = RouteTable::build(&line_graph());
        assert_eq!(table.hop_distance("sol", "sol"), Some(0));
        assert_eq!(table.hop_distance("sol", "beta"), Some(2));
        assert_eq!(table.hop_distance("beta", "sol"), Some(2));
    }

    #[test]
    fn disconnected_target_returns_none() {
        let graph = HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            ("alpha".to_string(), vec!["sol".to_string()]),
            ("beta".to_string(), vec![]),
        ]);
        let table = RouteTable::build(&graph);
        assert_eq!(table.path_hops("sol", "beta"), None);
        assert_eq!(table.hop_distance("sol", "beta"), None);
        assert_eq!(table.next_hop_toward("sol", "beta"), None);
    }

    #[test]
    fn start_equals_target_returns_empty_path() {
        let table = RouteTable::build(&line_graph());
        assert_eq!(table.path_hops("sol", "sol"), Some(Vec::new()));
    }

    #[test]
    fn unknown_node_returns_none() {
        let table = RouteTable::build(&HashMap::new());
        assert_eq!(table.path_hops("sol", "nowhere"), None);
        assert_eq!(table.hop_distance("sol", "nowhere"), None);
        // Same-system queries don't require the system to be on the map.
        assert_eq!(table.hop_distance("sol", "sol"), Some(0));
    }

    #[test]
    fn equal_length_routes_resolve_to_sorted_first_hop() {
        // Two equal-length paths: sol->a->c and sol->b->c.
        let graph = HashMap::from([
            ("sol".to_string(), vec!["b".to_string(), "a".to_string()]),
            ("a".to_string(), vec!["sol".to_string(), "c".to_string()]),
            ("b".to_string(), vec!["sol".to_string(), "c".to_string()]),
            ("c".to_string(), vec!["a".to_string(), "b".to_string()]),
        ]);
        let table = RouteTable::build(&graph);
        // Neighbors are visited in sorted order, so "a" wins regardless of
        // adjacency-list or hash ordering.
        assert_eq!(
            table.path_hops("sol", "c"),
            Some(vec!["a".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn weighted_routes_avoid_penalized_systems() {
        // Naked shortest path is sol->bad1->bad2->target (3 hops), but the
        // weighted cost prefers the longer safe path.
        let graph = HashMap::from([
            (
                "sol".to_string(),
                vec!["bad1".to_string(), "good".to_string()],
            ),
            (
                "bad1".to_string(),
                vec!["sol".to_string(), "bad2".to_string()],
            ),
            (
                "bad2".to_string(),
                vec!["bad1".to_string(), "target".to_string()],
            ),
            (
                "good".to_string(),
                vec!["sol".to_string(), "mid".to_string()],
            ),
            (
                "mid".to_string(),
                vec!["good".to_string(), "safe".to_string()],
            ),
            (
                "safe".to_string(),
                vec!["mid".to_string(), "target".to_string()],
            ),
            (
                "target".to_string(),
                vec!["bad2".to_string(), "safe".to_string()],
            ),
        ]);
        let table = RouteTable::build_with_penalties(
            &graph,
            &HashSet::from(["bad1".to_string(), "bad2".to_string(), "target".to_string()]),
        );

        assert_eq!(table.hop_distance("sol", "target"), Some(3));
        assert_eq!(table.path_cost("sol", "target"), Some(5));
        assert_eq!(
            table.naked_path_hops("sol", "target"),
            Some(vec![
                "bad1".to_string(),
                "bad2".to_string(),
                "target".to_string()
            ])
        );
        assert_eq!(
            table.path_hops("sol", "target"),
            Some(vec![
                "good".to_string(),
                "mid".to_string(),
                "safe".to_string(),
                "target".to_string()
            ])
        );
    }

    #[test]
    fn systems_within_hops_uses_naked_distance() {
        let graph = line_graph();
        let systems =
            RouteTable::systems_within_hops(&graph, &HashSet::from(["sol".to_string()]), 1);

        assert!(systems.contains("sol"));
        assert!(systems.contains("alpha"));
        assert!(!systems.contains("beta"));
    }

    #[test]
    fn directed_connections_are_respected() {
        let graph = HashMap::from([("sol".to_string(), vec!["alpha".to_string()])]);
        let table = RouteTable::build(&graph);
        assert_eq!(table.hop_distance("sol", "alpha"), Some(1));
        assert_eq!(table.hop_distance("alpha", "sol"), None);
    }
}
