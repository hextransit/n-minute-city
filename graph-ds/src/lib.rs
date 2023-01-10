mod hexagon_graph;
mod u64_graph;

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

impl<T: Eq + Hash + Copy + Send + Sync> Graph<T> {
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
    pub fn matrix_distance(&self, origins: Vec<T>, infinity: Option<f64>) -> Vec<Option<Vec<Arc<Node<T>>>>> {
        origins.par_iter().map(|s| self.bfs(*s, None)).collect()
    }

    /// perform a breadth first search on the graph
    /// * if `end` is None, the distance to all nodes will be recorded
    /// * if `end` is Some, only the distances on the backtraced path will be returned, 
    /// the nodes will be in the order of the path
    pub fn bfs(&self, start: T, end: Option<T>) -> Option<Vec<Arc<Node<T>>>> {
        
        let mut queue = Vec::new();
        let mut visited = Vec::new();
        let mut path = Vec::new();

        let start_node = self.node_map.get_by_left(&start).unwrap();
        // let end_node = self.node_map.get_by_left(&end).unwrap();

        queue.push(self.nodes[*start_node].as_ref().unwrap().clone());

        while !queue.is_empty() {
            let current_node = queue.remove(0);
            visited.push(current_node.id);

            if current_node.id == end {
                path.push(current_node);
                break;
            }

            let edges = self.edges.get(&current_node.id).unwrap();
            for edge in edges {
                let to_node = edge.to.upgrade().unwrap();
                if !visited.contains(&to_node.id) {
                    queue.push(to_node.clone());
                    path.push(to_node.clone());
                }
            }
        }

        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
}

impl<T: Eq + Hash + Copy + Send + Sync> Default for Graph<T> {
    fn default() -> Self {
        Self::new()
    }
}
