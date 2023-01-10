use crate::{Edge, Graph, Node};
use anyhow::Result;
use std::{sync::{Arc, Weak}, fmt::Display};

impl Graph<u64> {
    /// add an edge to the graph, if the nodes don't exist, they will be created
    pub fn build_and_add_egde(
        &mut self,
        from: &Node<u64>,
        to: &Node<u64>,
        weight: Option<f64>,
        capacity: Option<f64>,
    ) -> Result<bool> {
        // check if the nodes exist and if not, create them
        let start_node = match self.nodes.get(from.id as usize) {
            Some(Some(start_node)) => Arc::downgrade(start_node),
            _ => self.add_node(from)?,
        };
        let end_node = match self.nodes.get(to.id as usize) {
            Some(Some(end_node)) => Arc::downgrade(end_node),
            _ => self.add_node(to)?,
        };
        // create the edge
        // add the edge to the graph
        self.edges
            .entry(start_node.upgrade().unwrap().id)
            .or_insert_with(Vec::new)
            .push(Arc::new(Edge {
                from: start_node,
                to: end_node,
                weight,
                capacity,
            }));

        Ok(true)
    }

    /// add a node to the graph, changes the node ID to the index of the node in the vector
    pub fn add_node(&mut self, node: &Node<u64>) -> Result<Weak<Node<u64>>> {
        // the new node will be insterted at the end of the vector
        let node = Node {
            id: self.nodes.len() as u64,
            layer: node.layer,
        };
        self.nodes.push(Some(Arc::new(node)));
        if let Some(Some(node)) = self.nodes.last() {
            Ok(Arc::downgrade(node))
        } else {
            Err(anyhow::anyhow!("could not add node"))
        }
    }

    pub fn remove_node(&mut self, node: &Node<u64>) -> Result<bool> {
        // remove the node from the edge list and drop the node held in the arc
        let Some(node_ref) = self
            .nodes
            .get_mut(node.id as usize)
            .ok_or_else(|| anyhow::anyhow!("node not found"))? else {
                return Err(anyhow::anyhow!("node not found"));
            };
        self.edges.remove(&node_ref.id);
        self.nodes[node.id as usize] = None;
        Ok(true)
    }
}

impl Display for Edge<u64>{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Edge {{ from: {:?}, to: {:?}, weight: {:?}, capacity: {:?} }}",
            self.from.upgrade(),
            self.to.upgrade(),
            self.weight,
            self.capacity
        )
    }
}