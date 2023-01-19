mod hexagon_graph;
mod u64_graph;

use std::collections::VecDeque;
use std::hash::Hash;
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use bimap::BiMap;
use rayon::prelude::*;

#[derive(Debug)]
pub struct Graph<T> {
    pub nodes: Vec<Option<Arc<Node<T>>>>,
    pub edges: HashMap<T, Vec<Arc<Edge<T>>>>,
    pub node_map: BiMap<T, usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Node<T> {
    pub id: T,
    pub layer: Option<i32>,
    // pub name: String,
}

#[derive(Debug)]
pub struct Edge<T> {
    pub from: Weak<Node<T>>,
    pub to: Weak<Node<T>>,
    pub weight: Option<f64>,
    pub capacity: Option<f64>,
}

impl<T: Eq + Hash + Copy + Send + Sync + std::fmt::Debug> Graph<T> {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: HashMap::new(),
            node_map: BiMap::new(),
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
    ) -> Vec<HashMap<T, f64>> {
        origins
            .into_par_iter()
            .flat_map(|s| self.bfs(s, None).map(|res| res.1))
            .collect()
    }

    /// perform a breadth first search on the graph
    /// * if `end` is None, the distance to all nodes will be recorded
    /// * if `end` is Some, only the distances on the backtraced path will be returned,
    /// the nodes will be in the order of the path
    pub fn bfs(
        &self,
        start: T,
        end: Option<T>,
    ) -> anyhow::Result<(Option<Vec<T>>, HashMap<T, f64>)> {
        let mut q: VecDeque<(f64, &Edge<T>)> = VecDeque::new();
        let mut explored = HashMap::<T, bool>::new();
        let mut distances = HashMap::<T, f64>::new();
        let mut parents = HashMap::<T, T>::new();

        // let start_node = self.node_map.get_by_left(&start);
        println!("start node: {:?}", start);
        println!("nodes: {:?}", self.edges.get(&start));

        // get the edges from the start node
        explored.insert(start, true);
        distances.insert(start, 0.0);

        self.edges
            .get(&start)
            .ok_or_else(|| anyhow::anyhow!("start node not found in adjacency list"))?
            .iter()
            .for_each(|edge| {
                let edge = edge.as_ref();
                let edge_length = edge.weight.unwrap_or(1.0);
                q.push_back((edge_length, edge));
            });

        while !q.is_empty() {
            let (current_distance, current_egde) =
                q.pop_front().ok_or_else(|| anyhow::anyhow!("queue is empty"))?;

            // get the target of the current edge
            let Some(target) = current_egde.to.upgrade() else {
                continue
            };

            // record distance to target
            distances.insert(target.id, current_distance);

            // mark target as explored
            explored.insert(target.id, true);

            // record target parent
            parents.insert(target.id, current_egde.from.upgrade().unwrap().id);

            if let Some(end) = &end {
                if &target.id == end {
                    // found the target, backtrace the path
                    let mut path = Vec::<T>::new();
                    let mut current = target.id;
                    while let Some(parent) = parents.get(&current) {
                        path.push(*parent);
                        current = *parent;
                    }

                    return Ok((Some(path), distances));
                }
            }

            // we have not found the target, add unexplored edges from the target to the queue
            // check if there are any unexplored edges from the target
            if let Some(next_edges) = self.edges.get(&target.id) {
                next_edges.iter().for_each(|edge| {
                    let edge = edge.as_ref();
                    let edge_length = edge.weight.unwrap_or(1.0);
                    if let Some(edge_target) = edge.to.upgrade() {
                        if !explored.contains_key(&edge_target.id) {
                            q.push_back((current_distance + edge_length, edge));
                        }
                    }
                });
            }
            
        }

        Ok((None, distances))
    }
}

impl<T: Eq + Hash + Copy + Send + Sync + std::fmt::Debug> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}
