pub mod hexagon_graph;
pub mod u64_graph;

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashSet, VecDeque};
use std::hash::Hash;
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
    pub edges: Arc<RwLock<HashMap<usize, Vec<Edge>, nohash::BuildNoHashHasher<usize>>>>,
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

    pub fn nr_nodes(&self) -> usize {
        self.nodes.as_ref().read().unwrap().len()
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
                let res = self.build_and_add_egde(from.id, to.id, edge.weight, edge.capacity);
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
        capacity: Option<f64>,
    ) -> anyhow::Result<()> {
        let mut node_map = self.node_map.as_ref().write().unwrap();
        let mut node_list = self.nodes.as_ref().write().unwrap();

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
        // create the edge
        // add the edge to the graph
        self.edges
            .as_ref()
            .write()
            .unwrap()
            .entry(start_node_index)
            .or_default()
            .push(crate::Edge {
                from: start_node_index,
                to: end_node_index,
                weight,
                capacity,
            });

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
    pub fn matrix_bfs_distance(&self, origins: Vec<T>, force: bool) -> Vec<Vec<Option<f64>>> {
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
    /// * if `end` is Some, only the distance to the target will be returned,
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

            if let Some(end) = global_target_idx {
                if &current_target_idx == end {
                    // backtrace the path in parents
                    let path = self.backtrace(&parents, *end, *start_idx);

                    return Ok((path.ok(), vec![Some(current_distance)]));
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

    /// calculates the shortest path between two nodes using the A* algorithm, returns the path and the distance
    pub fn astar(
        &self,
        start: T,
        end: T,
        heuristic: impl Fn(&T, &T) -> f64,
    ) -> anyhow::Result<(Vec<T>, f64)> {
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

        let Some(start_idx) = node_map_access.get_by_left(&start) else {
            return Err(anyhow::anyhow!("start node not found in node map"));
        };
        let Some(end_idx) = node_map_access.get_by_left(&end) else {
            return Err(anyhow::anyhow!("end node not found in node map"));
        };

        g_score[*start_idx] = Some(0.0);
        q.push(Reverse(AStarNode {
            id: *start_idx,
            f_score: heuristic(&start, &end),
        }));

        while !q.is_empty() {
            let current = q.pop().ok_or(anyhow::anyhow!("queue was empty"))?.0;
            let current_idx = current.id;

            if current_idx == *end_idx {
                // found the target, backtrace the path
                let path = self.backtrace(&parents, *end_idx, *start_idx);
                return Ok((
                    path?,
                    g_score[*end_idx].ok_or(anyhow::anyhow!("target g score was not recorded"))?,
                ));
            }

            if let Some(next_edges) = edges_access.get(&current_idx) {
                for next_edge in next_edges.iter() {
                    let next_edge_target_idx = next_edge.to;
                    let tentative_g_score = g_score[current_idx]
                        .ok_or(anyhow::anyhow!("current g score was not recorded"))?
                        + next_edge.weight.unwrap_or(1.0);

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

        Err(anyhow::anyhow!("no path found"))
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
