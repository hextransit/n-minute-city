pub mod cell;

use std::{
    collections::HashSet,
    sync::RwLockWriteGuard,
};

use crate::{Graph, Node};
use anyhow::Result;
use bimap::BiHashMap;
use cell::Cell;

/// each node is a hexagon cell
/// this uses a simple hexagon grid, which does support layering
impl Graph<Cell> {
    /// add an edge to the graph, if the nodes don't exist, they will be created
    pub fn build_and_add_egde(
        &mut self,
        from: Cell,
        to: Cell,
        weight: Option<f64>,
        capacity: Option<f64>,
    ) -> Result<()> {
        // check if the nodes exist and if not, create them
        // map cell to node id

        let mut node_map = self.node_map.as_ref().write().unwrap();
        let mut node_list = self.nodes.as_ref().write().unwrap();

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
        node: Node<Cell>,
        node_list: &mut RwLockWriteGuard<Vec<Option<Node<Cell>>>>,
        node_map: &mut RwLockWriteGuard<BiHashMap<Cell, usize>>,
    ) -> Result<usize> {
        // the vector index will be saved in the node map
        let cell: Cell = node.id;
        let node_idx = node_list.len();
        // add node to the node_map
        node_map.insert(cell, node_idx);
        node_list.push(Some(node));
        Ok(node_idx)
    }
}

pub fn hexagon_graph_from_file(path: &str) -> Result<Graph<Cell>> {
    //reader
    let file = std::fs::File::open(path)?;
    let mut graph = Graph::<Cell>::new();
    let edges: HashSet<(Cell, Cell)> = rmp_serde::from_read(file)?;
    edges.iter().for_each(|(from, to)| {
        let res = graph.build_and_add_egde(*from, *to, Some(1.0), None);
        if res.is_err() {
            println!("error: {:?}", res);
        }
    });
    Ok(graph)
}
