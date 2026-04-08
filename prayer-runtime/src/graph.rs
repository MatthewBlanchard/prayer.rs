//! Shared graph/pathfinding helpers for runtime orchestration.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Return the first hop from `start` toward `target` if reachable.
pub(crate) fn next_hop_toward(
    connections: &HashMap<String, Vec<String>>,
    coordinates: &HashMap<String, (f64, f64)>,
    start: &str,
    target: &str,
) -> Option<String> {
    astar_shortest_path_hops(connections, coordinates, start, target)?
        .first()
        .cloned()
}

/// Return hop count distance between `start` and `target` if reachable.
pub(crate) fn hop_distance(
    connections: &HashMap<String, Vec<String>>,
    coordinates: &HashMap<String, (f64, f64)>,
    start: &str,
    target: &str,
) -> Option<usize> {
    Some(astar_shortest_path_hops(connections, coordinates, start, target)?.len())
}

/// Compute shortest path hops using A*.
/// The returned vector excludes `start` and includes `target`.
pub(crate) fn astar_shortest_path_hops(
    connections: &HashMap<String, Vec<String>>,
    coordinates: &HashMap<String, (f64, f64)>,
    start: &str,
    target: &str,
) -> Option<Vec<String>> {
    if start == target {
        return Some(Vec::new());
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<String, String> = HashMap::new();
    let mut g_score: HashMap<String, usize> = HashMap::new();

    g_score.insert(start.to_string(), 0);
    open.push(OpenNode {
        node: start.to_string(),
        g_cost: 0,
        f_cost: heuristic(coordinates, start, target),
    });

    while let Some(current) = open.pop() {
        if current.node == target {
            return rebuild_path(&came_from, start, target);
        }
        let current_g = *g_score.get(current.node.as_str()).unwrap_or(&usize::MAX);
        if current.g_cost > current_g {
            continue;
        }

        let mut neighbors = connections
            .get(current.node.as_str())
            .cloned()
            .unwrap_or_default();
        neighbors.sort();

        for neighbor in neighbors {
            let tentative_g = current.g_cost.saturating_add(1);
            let best_known = *g_score.get(neighbor.as_str()).unwrap_or(&usize::MAX);
            if tentative_g >= best_known {
                continue;
            }
            came_from.insert(neighbor.clone(), current.node.clone());
            g_score.insert(neighbor.clone(), tentative_g);
            let f = tentative_g as f64 + heuristic(coordinates, neighbor.as_str(), target);
            open.push(OpenNode {
                node: neighbor,
                g_cost: tentative_g,
                f_cost: f,
            });
        }
    }

    None
}

fn heuristic(coordinates: &HashMap<String, (f64, f64)>, a: &str, b: &str) -> f64 {
    let Some((ax, ay)) = coordinates.get(a) else {
        return 0.0;
    };
    let Some((bx, by)) = coordinates.get(b) else {
        return 0.0;
    };
    let dx = ax - bx;
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt()
}

fn rebuild_path(
    came_from: &HashMap<String, String>,
    start: &str,
    target: &str,
) -> Option<Vec<String>> {
    let mut path = Vec::new();
    let mut cursor = target.to_string();
    path.push(cursor.clone());

    while cursor != start {
        let prev = came_from.get(cursor.as_str())?.clone();
        cursor = prev;
        if cursor != start {
            path.push(cursor.clone());
        }
    }

    path.reverse();
    Some(path)
}

#[derive(Debug, Clone)]
struct OpenNode {
    node: String,
    g_cost: usize,
    f_cost: f64,
}

impl PartialEq for OpenNode {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node && self.g_cost == other.g_cost && self.f_cost == other.f_cost
    }
}

impl Eq for OpenNode {}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        match other
            .f_cost
            .partial_cmp(&self.f_cost)
            .unwrap_or(Ordering::Equal)
        {
            Ordering::Equal => match other.g_cost.cmp(&self.g_cost) {
                Ordering::Equal => other.node.cmp(&self.node),
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn astar_shortest_path_hops_returns_hop_sequence() {
        let graph = HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            (
                "alpha".to_string(),
                vec!["sol".to_string(), "beta".to_string()],
            ),
            ("beta".to_string(), vec!["alpha".to_string()]),
        ]);
        let coords = HashMap::from([
            ("sol".to_string(), (0.0, 0.0)),
            ("alpha".to_string(), (1.0, 0.0)),
            ("beta".to_string(), (2.0, 0.0)),
        ]);
        let hops = astar_shortest_path_hops(&graph, &coords, "sol", "beta").expect("path");
        assert_eq!(hops, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn next_hop_toward_returns_first_hop() {
        let graph = HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            (
                "alpha".to_string(),
                vec!["sol".to_string(), "beta".to_string()],
            ),
            ("beta".to_string(), vec!["alpha".to_string()]),
        ]);
        let coords = HashMap::new();
        assert_eq!(
            next_hop_toward(&graph, &coords, "sol", "beta"),
            Some("alpha".to_string())
        );
    }

    #[test]
    fn hop_distance_counts_edges() {
        let graph = HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            (
                "alpha".to_string(),
                vec!["sol".to_string(), "beta".to_string()],
            ),
            ("beta".to_string(), vec!["alpha".to_string()]),
        ]);
        let coords = HashMap::new();
        assert_eq!(hop_distance(&graph, &coords, "sol", "sol"), Some(0));
        assert_eq!(hop_distance(&graph, &coords, "sol", "beta"), Some(2));
        assert_eq!(hop_distance(&graph, &coords, "beta", "sol"), Some(2));
    }

    #[test]
    fn astar_returns_none_for_disconnected_graph() {
        let graph = HashMap::from([
            ("sol".to_string(), vec!["alpha".to_string()]),
            ("alpha".to_string(), vec!["sol".to_string()]),
            ("beta".to_string(), vec![]),
        ]);
        let coords = HashMap::new();
        assert_eq!(
            astar_shortest_path_hops(&graph, &coords, "sol", "beta"),
            None
        );
    }

    #[test]
    fn astar_start_equals_target_returns_empty_path() {
        let graph = HashMap::from([("sol".to_string(), vec!["alpha".to_string()])]);
        let coords = HashMap::new();
        let hops = astar_shortest_path_hops(&graph, &coords, "sol", "sol").expect("path");
        assert!(hops.is_empty());
    }

    #[test]
    fn astar_unknown_node_returns_none() {
        let graph: HashMap<String, Vec<String>> = HashMap::new();
        let coords = HashMap::new();
        assert_eq!(
            astar_shortest_path_hops(&graph, &coords, "sol", "nowhere"),
            None
        );
    }

    #[test]
    fn astar_tie_breaking_is_deterministic() {
        // Two equal-length paths: sol->a->c and sol->b->c
        let graph = HashMap::from([
            ("sol".to_string(), vec!["a".to_string(), "b".to_string()]),
            ("a".to_string(), vec!["sol".to_string(), "c".to_string()]),
            ("b".to_string(), vec!["sol".to_string(), "c".to_string()]),
            ("c".to_string(), vec!["a".to_string(), "b".to_string()]),
        ]);
        let coords = HashMap::new();
        let first = astar_shortest_path_hops(&graph, &coords, "sol", "c").expect("path1");
        let second = astar_shortest_path_hops(&graph, &coords, "sol", "c").expect("path2");
        assert_eq!(first, second, "tie-breaking must be deterministic");
        assert_eq!(first.len(), 2);
        assert_eq!(first.last().map(String::as_str), Some("c"));
    }

    #[test]
    fn hop_distance_returns_none_for_unreachable_target() {
        let graph = HashMap::from([("sol".to_string(), vec![]), ("beta".to_string(), vec![])]);
        let coords = HashMap::new();
        assert_eq!(hop_distance(&graph, &coords, "sol", "beta"), None);
    }
}
