pub mod hexagon_graph;
pub mod u64_graph;

use std::cmp::{Ordering, Reverse};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, BinaryHeap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::RwLockWriteGuard;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use bimap::{BiHashMap, BiMap};

use rayon::prelude::*;

#[allow(clippy::type_complexity)]
#[derive(Debug)]
pub struct Graph<T> {
    pub nodes: Arc<RwLock<Vec<Option<Node<T>>>>>,
    pub edges: Arc<RwLock<HashMap<usize, HashSet<Edge>, nohash::BuildNoHashHasher<usize>>>>,
    pub node_map: Arc<RwLock<BiMap<T, usize>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub id: T,
    pub layer: Option<i32>,
}

/// an edge is decribed by the index of the source and target nodes
///
/// there are optional fields for weight and capacity
#[derive(Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub weight: Option<f64>,
    pub weight_list: Option<Vec<f64>>,
    pub capacity: Option<f64>,
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from && self.to == other.to
    }
}

impl Eq for Edge {}

impl Hash for Edge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.from.hash(state);
        0x0.hash(state);
        self.to.hash(state);
    }
}

impl Edge {
    pub fn new(from: usize, to: usize, weight: Option<f64>, capacity: Option<f64>) -> Self {
        Self {
            from,
            to,
            weight,
            weight_list: None,
            capacity,
        }
    }
}

impl<T: Eq + Hash + Copy + Send + Sync + Ord + std::fmt::Debug> Graph<T> {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(Vec::new())),
            edges: Arc::new(RwLock::new(HashMap::default())),
            node_map: Arc::new(RwLock::new(BiMap::new())),
        }
    }

    pub fn nr_nodes(&self) -> usize {
        self.nodes.as_ref().read().unwrap().len()
    }

    pub fn node_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        let btree = BTreeSet::from_iter(
            self.nodes
                .as_ref()
                .read()
                .unwrap()
                .iter()
                .flatten()
                .map(|node| node.id),
        );
        btree.hash(&mut hasher);
        hasher.finish()
    }

    pub fn get_random_node(&self) -> Option<T> {
        let nodes = self.nodes.as_ref().read().unwrap();
        let index = rand::random::<usize>() % nodes.len();
        if let Some(Some(node)) = nodes.get(index) {
            Some(node.id)
        } else {
            None
        }
    }

    /// adds all nodes and edges from other to self, connects the graphs where they share nodes. Does not remove any nodes or edges.
    pub fn merge(&mut self, other: &mut Self) -> anyhow::Result<()> {
        let other_nodes = other.nodes.as_ref().read().unwrap();
        let other_edges = other.edges.as_ref().read().unwrap();

        other_edges.values().for_each(|edges| {
            for edge in edges.iter() {
                let Some(from) = other_nodes.get(edge.from).unwrap() else {
                    continue;
                };
                let Some(to) = other_nodes.get(edge.to).unwrap() else {
                    continue;
                };
                let res = self.build_and_add_egde(
                    from.id,
                    to.id,
                    edge.weight,
                    edge.weight_list.clone(),
                    edge.capacity,
                );
                if res.is_err() {
                    println!("error: {res:?}");
                }
            }
        });

        Ok(())
    }

    /// add an edge to the graph
    /// * if the nodes do not exist, they will be created
    /// * and edge can have a weight and capacity
    pub fn build_and_add_egde(
        &mut self,
        from: T,
        to: T,
        weight: Option<f64>,
        weight_list: Option<Vec<f64>>,
        capacity: Option<f64>,
    ) -> anyhow::Result<()> {
        let Ok( mut node_map) = self.node_map.as_ref().write() else {
            return Err(anyhow::anyhow!("could not get write lock on node_map"));
        };
        let Ok( mut node_list) = self.nodes.as_ref().write() else {
            return Err(anyhow::anyhow!("could not get write lock on node_list"));
        };

        // check if the nodes exist and if not, create them
        let start_node_index = match node_map.get_by_left(&from) {
            Some(start_node_index) => *start_node_index,
            _ => self.add_node(
                Node {
                    id: from,
                    layer: None,
                },
                &mut node_list,
                &mut node_map,
            )?,
        };
        let end_node_index = match node_map.get_by_left(&to) {
            Some(end_node_index) => *end_node_index,
            _ => self.add_node(
                Node {
                    id: to,
                    layer: None,
                },
                &mut node_list,
                &mut node_map,
            )?,
        };

        if start_node_index == end_node_index {
            return Ok(());
        };

        let Ok( mut edges) = self.edges.as_ref().write() else {
            return Err(anyhow::anyhow!("could not get write lock on edges"));
        };
        // create the edge
        // add the edge to the graph
        let new_edge = crate::Edge {
            from: start_node_index,
            to: end_node_index,
            weight,
            weight_list,
            capacity,
        };

        if let Some(existing_edge) = edges
                    .entry(start_node_index)
                    .or_default()
                    .get(&new_edge) {
            if existing_edge.weight.unwrap_or(60.0) > new_edge.weight.unwrap_or(60.0) {
                edges
                    .entry(start_node_index)
                    .or_default()
                    .insert(new_edge);
            }
        } else {
            edges
                .entry(start_node_index)
                .or_default()
                .insert(new_edge);
        }
        Ok(())
    }

    /// add a node to the graph, changes the node ID to the index of the node in the vector
    pub fn add_node(
        &self,
        node: Node<T>,
        node_list: &mut RwLockWriteGuard<Vec<Option<Node<T>>>>,
        node_map: &mut RwLockWriteGuard<BiHashMap<T, usize>>,
    ) -> anyhow::Result<usize> {
        // the vector index will be saved in the node map
        //let cell: Cell = node.id;
        let node_idx = node_list.len();
        // add node to the node_map
        node_map.insert(node.id, node_idx);
        node_list.push(Some(node));
        Ok(node_idx)
    }

    /// removes a directed edge from the graph
    pub fn remove_edge(&mut self, from: T, to: T) -> anyhow::Result<()> {
        let node_map = self.node_map.as_ref().read().unwrap();
        let mut edges = self.edges.as_ref().write().unwrap();

        // get node indices
        let from = node_map
            .get_by_left(&from)
            .ok_or(anyhow::anyhow!("node not found"))?;
        let to = node_map
            .get_by_left(&to)
            .ok_or(anyhow::anyhow!("node not found"))?;

        // find the edge in edges
        edges.entry(*from).and_modify(|edges| {
            edges.retain(|edge| edge.to != *to);
        });

        Ok(())
    }

    /// calculate the directed distance from a set of origins to all nodes in the graph
    /// * if `infinity` is None, the distance to all nodes will be recorded, otherwise the calculation is cutoff at `infinity`
    ///
    /// this function is parallelized using rayon
    pub fn matrix_bfs_distance(
        &self,
        origins: &Vec<T>,
        destinations: Option<&Vec<T>>,
        force: bool,
    ) -> HashMap<T, anyhow::Result<Vec<Option<f64>>>> {
        let map_func = |s: &T| (*s, self.bfs(s, None, destinations).map(|res| res.1));
        if force {
            origins.into_par_iter().map(map_func).collect()
        } else {
            // removes duplicates before iteration
            origins
                .iter()
                .collect::<HashSet<&T>>()
                .into_par_iter()
                .map(map_func)
                .collect()
        }
    }

    #[allow(clippy::type_complexity)]
    /// perform a breadth first search on the graph
    /// * if `end` is None, the distance to all nodes will be recorded
    /// * if `end` is Some, only the distance to the target will be returned,
    /// the nodes will be in the order of the path
    pub fn bfs(
        &self,
        start: &T,
        end: Option<&T>,
        end_list: Option<&Vec<T>>,
    ) -> anyhow::Result<(Option<Vec<T>>, Vec<Option<f64>>)> {
        let mut q: VecDeque<(f64, &Edge)> = VecDeque::new();

        let nodes_access = self.nodes.read().unwrap();
        let nr_nodes = nodes_access.len();
        let mut explored = vec![false; nr_nodes];
        let mut distances: Vec<Option<f64>> = vec![None; nr_nodes];
        let mut parents: Vec<Option<usize>> = vec![None; nr_nodes];

        // get the edges from the start node
        let node_map_access = self.node_map.as_ref().read().unwrap();
        let start_idx = *node_map_access
            .get_by_left(start)
            .ok_or_else(|| anyhow::anyhow!("start node {start:?} not found in node map"))?;

        let global_target_idx = if let Some(end) = &end {
            node_map_access.get_by_left(end)
        } else {
            None
        };

        let mut end_distances: HashMap<T, f64> = HashMap::new();

        let global_target_list = end_list.as_ref().map(|end_list| {
            end_list
                .iter()
                .filter_map(|end| node_map_access.get_by_left(end))
                .collect::<HashSet<_>>()
        });

        explored[start_idx] = true;
        distances[start_idx] = Some(0.0);

        let edges_access = self.edges.as_ref().read().unwrap();

        edges_access
            .get(&start_idx)
            .ok_or_else(|| anyhow::anyhow!("start node not found in adjacency list"))?
            .iter()
            .for_each(|edge| {
                let edge_length = edge.weight.unwrap_or(1.0);
                explored[edge.to] = true;
                distances[edge.to] = Some(1.0);
                parents[edge.to] = Some(start_idx);
                q.push_back((edge_length, edge));
            });

        if let Some(target_list) = &global_target_list {
            if target_list.contains(&start_idx) {
                end_distances.insert(*start, 0.0);
            }
        }

        while !q.is_empty() {
            let (current_distance, current_egde) = q
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("queue is empty"))?;

            // get the target of the current edge
            let current_target_idx = current_egde.to;

            if let Some(end) = global_target_idx {
                if &current_target_idx == end {
                    // backtrace the path in parents
                    let path = self.backtrace(&parents, *end, start_idx);

                    return Ok((path.ok(), vec![Some(current_distance)]));
                }
            }

            // we have not found the target, add unexplored edges from the target to the queue
            // check if there are any unexplored edges from the target
            if let Some(next_edges) = edges_access.get(&current_target_idx) {
                for next_edge in next_edges.iter() {
                    let next_edge_target_idx = next_edge.to;
                    if !explored[next_edge_target_idx] {
                        explored[next_edge_target_idx] = true;
                        distances[next_edge_target_idx] = Some(current_distance + 1.0);
                        parents[next_edge_target_idx] = Some(current_target_idx);

                        q.push_back((current_distance + 1.0, next_edge));
                    }
                }
            }
        }

        if end_list.is_some() {
            let distances = end_distances.into_values().map(Some).collect();
            return Ok((None, distances));
        }
        if end.is_some() {
            return Ok((None, vec![None]));
        }

        Ok((None, distances))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn matrix_astar_distance(
        &self,
        origins: &Vec<T>,
        destinations: Option<&Vec<T>>,
        force: bool,
        weight_list_index: Option<usize>,
        infinity: Option<f64>,
        dynamic_infinity: Option<bool>,
        heuristic: impl Fn(&T, &T) -> f64 + Send + Sync + Copy,
    ) -> HashMap<T, anyhow::Result<Vec<Option<f64>>>> {
        let map_func = |s: &T| {
            (
                *s,
                self.astar(
                    s,
                    None,
                    destinations,
                    infinity,
                    dynamic_infinity,
                    weight_list_index,
                    heuristic,
                )
                .map(|res| res.distances),
            )
        };
        if force {
            origins.into_par_iter().map(map_func).collect()
        } else {
            // removes duplicates before iteration
            origins
                .iter()
                .collect::<HashSet<&T>>()
                .into_par_iter()
                .map(map_func)
                .collect()
        }
    }

    /// calculates the shortest path between two nodes using the A* algorithm, returns the path and the distance
    #[allow(clippy::too_many_arguments)]
    pub fn astar(
        &self,
        start: &T,
        end: Option<&T>,
        end_list: Option<&Vec<T>>,
        infinity: Option<f64>,
        dynamic_infinity: Option<bool>,
        weight_list_index: Option<usize>,
        heuristic: impl Fn(&T, &T) -> f64,
    ) -> anyhow::Result<AStarResult<T>> {
        #[derive(Debug, Clone, PartialEq)]
        struct AStarNode {
            id: usize,
            f_score: f64,
        }

        impl Eq for AStarNode {}

        impl Ord for AStarNode {
            fn cmp(&self, other: &Self) -> Ordering {
                self.f_score.partial_cmp(&other.f_score).unwrap()
            }
        }

        impl PartialOrd for AStarNode {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let mut q: BinaryHeap<Reverse<AStarNode>> = BinaryHeap::new();
        let nr_nodes = self.nodes.read().unwrap().len();
        let mut parents: Vec<Option<usize>> = vec![None; nr_nodes];
        let mut g_score: Vec<Option<f64>> = vec![None; nr_nodes];

        let node_map_access = self.node_map.as_ref().read().unwrap();
        let node_list_access = self.nodes.as_ref().read().unwrap();
        let edges_access = self.edges.as_ref().read().unwrap();

        let start_idx = *node_map_access
            .get_by_left(start)
            .ok_or_else(|| anyhow::anyhow!("start node {start:?} not found in node map"))?;

        let target_list = if let Some(end_list) = end_list {
            if end_list.is_empty() {
                return Err(anyhow::anyhow!("no end node provided"));
            }
            end_list.clone()
        } else if let Some(end) = end {
            vec![*end]
        } else {
            return Err(anyhow::anyhow!("no end node provided"));
        };

        let mut infinity = infinity.unwrap_or(std::f64::INFINITY);

        let target_idx_list = target_list
            .iter()
            .filter_map(|end| node_map_access.get_by_left(end))
            .collect::<Vec<_>>();

        let mut target_idx_set = target_idx_list.iter().cloned().collect::<HashSet<_>>();

        let is_single_target = end.is_some();

        g_score[start_idx] = Some(0.0);
        q.push(Reverse(AStarNode {
            id: start_idx,
            f_score: heuristic(start, &target_list[0]),
        }));

        while !q.is_empty() {
            let current = q.pop().ok_or(anyhow::anyhow!("queue was empty"))?.0;
            let current_idx = current.id;

            if g_score[current_idx].unwrap_or(0.0) > infinity {
                continue;
            }

            if target_idx_set.contains(&current_idx) {
                // found the target, backtrace the path
                if dynamic_infinity.unwrap_or(false) {
                    infinity = g_score[current_idx].unwrap_or(f64::INFINITY);
                }

                target_idx_set.remove(&current_idx);
                if is_single_target {
                    let path = self.backtrace(&parents, current_idx, start_idx)?;
                    return Ok(AStarResult {
                        path: Some(path),
                        single_target: is_single_target,
                        distances: vec![g_score[current_idx]],
                    });
                } else if target_idx_set.is_empty() {
                    return Ok(AStarResult {
                        path: None,
                        single_target: is_single_target,
                        distances: target_idx_list
                            .into_iter()
                            .map(|idx| g_score[*idx])
                            .collect::<Vec<_>>(),
                    });
                }
            }

            if let Some(next_edges) = edges_access.get(&current_idx) {
                for next_edge in next_edges.iter() {
                    let next_edge_target_idx = next_edge.to;
                    let tentative_g_score = if let (Some(weight_list), Some(list_idx)) =
                        (&next_edge.weight_list, weight_list_index)
                    {
                        g_score[current_idx]
                            .ok_or(anyhow::anyhow!("current g score was not recorded"))?
                            + weight_list[list_idx]
                    } else {
                        g_score[current_idx]
                            .ok_or(anyhow::anyhow!("current g score was not recorded"))?
                            + next_edge.weight.unwrap_or(1.0)
                    };
                    if g_score[next_edge_target_idx].is_none()
                        || tentative_g_score < g_score[next_edge_target_idx].unwrap()
                    {
                        parents[next_edge_target_idx] = Some(current_idx);
                        g_score[next_edge_target_idx] = Some(tentative_g_score);
                        q.push(Reverse(AStarNode {
                            id: next_edge_target_idx,
                            f_score: tentative_g_score
                                + heuristic(
                                    &node_list_access[next_edge_target_idx].as_ref().unwrap().id,
                                    &node_list_access[current_idx].as_ref().unwrap().id,
                                ),
                        }));
                    }
                }
            }
        }

        if end_list.is_some() {
            Ok(AStarResult {
                path: None,
                single_target: is_single_target,
                distances: target_idx_list
                    .into_iter()
                    .map(|idx| g_score[*idx])
                    .collect::<Vec<_>>(),
            })
        } else {
            Err(anyhow::anyhow!("no path found"))
        }
    }

    pub fn backtrace(
        &self,
        parents: &[Option<usize>],
        target: usize,
        start: usize,
    ) -> anyhow::Result<Vec<T>> {
        let node_map_access = self.node_map.as_ref().read().unwrap();
        let mut path = Vec::new();
        let mut current = target;

        loop {
            let Some(node) = node_map_access.get_by_right(&current) else {
                println!("[backtrace] {current} is not in node map");
                break;
            };
            path.push(*node);
            if current == start {
                println!("[backtrace] found start node");
                break;
            }
            if let Some(parent) = parents[current] {
                current = parent;
            } else {
                println!("[backtrace] no parent found for {current}");
                break;
            }
        }

        path.reverse();
        Ok(path)
    }
}

#[derive(Debug)]
pub struct AStarResult<T> {
    pub path: Option<Vec<T>>,
    pub single_target: bool,
    pub distances: Vec<Option<f64>>,
}

impl<T: Eq + Hash + Copy + Send + Ord + Sync + std::fmt::Debug> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "pyo3")]
#[pyo3::pymodule]
fn graph_ds(_py: pyo3::Python, m: &pyo3::types::PyModule) -> pyo3::PyResult<()> {
    m.add_class::<hexagon_graph::PyH3Graph>()?;
    m.add_class::<hexagon_graph::PyCellGraph>()?;

    Ok(())
}
