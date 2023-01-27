pub mod hexagon_graph;
pub mod u64_graph;

use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use bimap::BiMap;
use rayon::prelude::*;

#[derive(Debug)]
pub struct Graph<T> {
    pub nodes: Arc<RwLock<Vec<Option<Node<T>>>>>,
    pub edges: Arc<RwLock<HashMap<usize, Vec<Edge>, nohash::BuildNoHashHasher<usize>>>>,
    pub node_map: Arc<RwLock<BiMap<T, usize>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub id: T,
    pub layer: Option<i32>,
    // pub name: String,
}

/// an edge is decribed by the index of the source and target nodes
///
/// there are optional fields for weight and capacity
#[derive(Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub weight: Option<f64>,
    pub capacity: Option<f64>,
}

impl<T: Eq + Hash + Copy + Send + Sync + std::fmt::Debug> Graph<T> {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(Vec::new())),
            edges: Arc::new(RwLock::new(HashMap::default())),
            node_map: Arc::new(RwLock::new(BiMap::new())),
        }
    }

    /// calculate the directed distance from a set of origins to all nodes in the graph
    /// * if `infinity` is None, the distance to all nodes will be recorded, otherwise the calculation is cutoff at `infinity`
    ///
    /// this function is parallelized using rayon
    pub fn matrix_bfs_distance(
        &self,
        origins: Vec<T>,
        _infinity: Option<f64>,
        force: bool,
    ) -> Vec<Vec<Option<f64>>> {
        if force {
            origins
                .into_par_iter()
                .flat_map(|s| self.bfs(s, None).map(|res| res.1))
                .collect()
        } else {
            // removes duplicates before iteration
            origins
                .into_iter()
                .collect::<HashSet<T>>()
                .into_par_iter()
                .flat_map(|s| self.bfs(s, None).map(|res| res.1))
                .collect()
        }
    }

    #[allow(clippy::type_complexity)]
    /// perform a breadth first search on the graph
    /// * if `end` is None, the distance to all nodes will be recorded
    /// * if `end` is Some, only the distances on the visited nodes will be returned,
    /// the nodes will be in the order of the path
    pub fn bfs(
        &self,
        start: T,
        end: Option<T>,
    ) -> anyhow::Result<(Option<Vec<T>>, Vec<Option<f64>>)> {
        let mut q: VecDeque<(f64, &Edge)> = VecDeque::new();
        let nr_nodes = self.nodes.read().unwrap().len();
        let mut explored = vec![false; nr_nodes];
        let mut distances: Vec<Option<f64>> = vec![None; nr_nodes];
        let mut parents: Vec<Option<usize>> = vec![None; nr_nodes];

        // get the edges from the start node
        let node_map_access = self.node_map.as_ref().read().unwrap();
        let Some(start_idx) = node_map_access.get_by_left(&start) else {
            return Err(anyhow::anyhow!("start node not found in node map"));
        };

        let global_target_idx = if let Some(end) = &end {
            node_map_access.get_by_left(end)
        } else {
            None
        };

        explored[*start_idx] = true;
        distances[*start_idx] = Some(0.0);

        let edges_access = self.edges.as_ref().read().unwrap();

        edges_access
            .get(start_idx)
            .ok_or_else(|| anyhow::anyhow!("start node not found in adjacency list"))?
            .iter()
            .for_each(|edge| {
                let edge_length = edge.weight.unwrap_or(1.0);
                explored[edge.to] = true;
                distances[edge.to] = Some(edge_length);
                q.push_back((edge_length, edge));
            });

        while !q.is_empty() {
            let (current_distance, current_egde) = q
                .pop_front()
                .ok_or_else(|| anyhow::anyhow!("queue is empty"))?;


            // get the target of the current edge
            let current_target_idx = current_egde.to;

            // distances[current_target_idx] = Some(current_distance + current_egde.weight.unwrap_or(1.0));
            // parents[current_target_idx] = Some(current_egde.from);


            if let Some(end) = global_target_idx {
                if &current_target_idx == end {
                    // found the target, backtrace the path
                    println!("found target: {:?}", current_target_idx);
                    println!("distance: {}, backtracing path", current_distance);

                    // parents[*end] = Some(current_target_idx);
                    // backtrace the path in parents
                    let path = self.backtrace(&parents, *end, *start_idx);

                    return Ok((path.ok(), distances));
                }
            }

            // we have not found the target, add unexplored edges from the target to the queue
            // check if there are any unexplored edges from the target
            if let Some(next_edges) = edges_access.get(&current_target_idx) {
                for next_edge in next_edges.iter() {
                    let next_edge_target_idx = next_edge.to;
                    if !explored[next_edge_target_idx] {
                        let next_edge_length = next_edge.weight.unwrap_or(1.0);

                        explored[next_edge_target_idx] = true;
                        distances[next_edge_target_idx] = Some(current_distance + next_edge_length);
                        parents[next_edge_target_idx] = Some(current_egde.to);

                        q.push_back((current_distance + next_edge_length, next_edge));
                    }
                }
            }
        }

        Ok((None, distances))
    }

    pub fn backtrace(
        &self,
        parents: &Vec<Option<usize>>,
        target: usize,
        start: usize,
    ) -> anyhow::Result<Vec<T>> {
        let node_map_access = self.node_map.as_ref().read().unwrap();
        let mut path = Vec::new();
        let mut current = target;
        while current != start {
            let Some(node) = node_map_access.get_by_right(&current) else {
                break;
            };
            path.push(*node);
            if let Some(parent) = parents[current] {
                current = parent;
            } else {
                break;
            }
        }
        Ok(path)
    }
}

impl<T: Eq + Hash + Copy + Send + Sync + std::fmt::Debug> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}
