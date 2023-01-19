pub mod cell;

use std::{sync::{Arc, Weak}, collections::HashSet};

use crate::{Graph, Node};
use anyhow::Result;
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

        let start_node = match self.node_map.get_by_left(&from) {
            Some(start_node_index) => {
                Arc::downgrade(self.nodes[*start_node_index].as_ref().unwrap())
            }
            _ => self.add_node(Node {
                id: from,
                layer: None,
            })?,
        };
        let end_node = match self.node_map.get_by_left(&to) {
            Some(end_node_index) => Arc::downgrade(self.nodes[*end_node_index].as_ref().unwrap()),
            _ => self.add_node(Node {
                id: to,
                layer: None,
            })?,
        };

        // create the edge
        // add the edge to the graph
        self.edges
            .entry(start_node.upgrade().unwrap().id)
            .or_insert_with(Vec::new)
            .push(Arc::new(crate::Edge {
                from: start_node,
                to: end_node,
                weight,
                capacity,
            }));

        Ok(())
    }

    /// add a node to the graph, changes the node ID to the index of the node in the vector
    pub fn add_node(&mut self, node: Node<Cell>) -> Result<Weak<Node<Cell>>> {
        // the vector index will be saved in the node map
        let cell: Cell = node.id;
        let node_arc = Arc::new(node);
        let node_weak = Arc::downgrade(&node_arc);
        let node_id = self.nodes.len();
        // add node to the node_map
        self.node_map.insert(cell, node_id);
        self.nodes.push(Some(node_arc));

        Ok(node_weak)
    }
}

pub fn hexagon_graph_from_file(path: &str) -> Result<Graph<Cell>> {
    //reader
    let file = std::fs::File::open(path)?;
    let mut graph = Graph::<Cell>::new();
    let edges : HashSet<(Cell, Cell)> = rmp_serde::from_read(file)?;
    edges.iter().for_each(|(from, to)| {
        let res = graph.build_and_add_egde(*from, *to, Some(1.0), None);
        if res.is_err() {
            println!("error: {:?}", res);
        }
    });
    Ok(graph)
}