pub mod cell;

use std::collections::HashSet;

use crate::Graph;
use anyhow::Result;
use cell::Cell;

/// each node is a hexagon cell
/// this uses a simple hexagon grid, which does support layering
impl Graph<Cell> {
    // add an edge to the graph, if the nodes don't exist, they will be created
    
}

pub fn hexagon_graph_from_file(path: &str) -> Result<Graph<Cell>> {
    //reader
    let file = std::fs::File::open(path)?;
    let mut graph = Graph::<Cell>::new();
    let edges: HashSet<(Cell, Cell)> = rmp_serde::from_read(file)?;
    edges.iter().for_each(|(from, to)| {
        let res = graph.build_and_add_egde(*from, *to, Some(1.0), None);
        if res.is_err() {
            println!("error: {res:?}");
        }
    });
    Ok(graph)
}
