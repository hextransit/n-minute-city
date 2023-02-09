pub mod cell;
pub mod gtfs;
pub mod h3cell;
pub mod osm;

use std::collections::HashSet;

use crate::{Edge, Graph};
use cell::Cell;

use self::{cell::Direction, h3cell::H3Cell};

/// each node is a hexagon cell
/// this uses a simple hexagon grid, which does support layering
impl Graph<Cell> {
    /// connect a cell with a given list of direct neighbors, create the nodes in the graph if needed
    pub fn connect_cell(
        &mut self,
        cell: Cell,
        neighbors: &[Direction],
        weight: Option<f64>,
    ) -> anyhow::Result<()> {
        for neighbor in neighbors
            .iter()
            .map(|direction| cell.get_neighbor(*direction))
        {
            self.build_and_add_egde(cell, neighbor, weight, None)?
        }

        Ok(())
    }
}

pub fn hexagon_graph_from_file(path: &str) -> anyhow::Result<Graph<Cell>> {
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

pub fn h3_network_from_osm(
    osm_url: &str,
    osm_filter: Vec<String>,
) -> anyhow::Result<Graph<H3Cell>> {
    todo!()
}

pub fn h3_network_from_gtfs(gtfs_url: &str) -> anyhow::Result<Graph<H3Cell>> {
    let edge_data = gtfs::process_gtfs(gtfs_url, h3o::Resolution::Twelve)?;

    let mut graph = Graph::<H3Cell>::new();
    for ((layer, from, to), weight) in edge_data {
        let from_cell = H3Cell {
            cell: from,
            layer: layer as i16,
        };
        let to_cell = H3Cell {
            cell: to,
            layer: layer as i16,
        };
        let base_cell = H3Cell {
            cell: from,
            layer: -1,
        };
        // the transit edge
        graph.build_and_add_egde(from_cell, to_cell, Some(weight), None)?;
        // connections from the base layer
        graph.build_and_add_egde(base_cell, from_cell, Some(1.0), None)?;
        graph.build_and_add_egde(base_cell, to_cell, Some(1.0), None)?;
        // connections to the base layer
        graph.build_and_add_egde(from_cell, base_cell, Some(1.0), None)?;
        graph.build_and_add_egde(to_cell, base_cell, Some(1.0), None)?;
    }
    Ok(graph)
}

/// each node is a H3 hexagon cell
/// this uses a H3 hexagon grid, with additional layering
impl Graph<H3Cell> {
    /// return the H3 space equavalent of an edge. The H3 space does not support layering
    pub fn get_h3_edge(&self, edge: Edge) -> anyhow::Result<h3o::DirectedEdgeIndex> {
        let node_map = self.node_map.as_ref().read().unwrap();
        let start = node_map
            .get_by_right(&edge.from)
            .ok_or(anyhow::anyhow!("start node not found"))?;

        let end = node_map
            .get_by_right(&edge.to)
            .ok_or(anyhow::anyhow!("end node not found"))?;

        start
            .cell
            .edge(end.cell)
            .ok_or(anyhow::anyhow!("nodes are not neighbors in the H3 space"))
    }
}
