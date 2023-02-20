pub mod cell;
pub mod gtfs;
pub mod h3cell;
pub mod osm;

use std::{collections::HashSet, sync::RwLockReadGuard, time::Instant};

use crate::{Edge, Graph};
use bimap::BiHashMap;
use cell::Cell;

use self::{
    cell::Direction,
    h3cell::H3Cell,
    osm::{process_osm_pbf, OSMLayer},
};

use pyo3::prelude::*;

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

pub fn h3_network_from_osm(osm_url: &str, layer: OSMLayer) -> anyhow::Result<Graph<H3Cell>> {
    let edge_data = process_osm_pbf(osm_url, layer, h3o::Resolution::Twelve)?;

    let mut graph = Graph::<H3Cell>::new();

    let layer_id: i16 = match layer {
        OSMLayer::Cycling => -2,
        OSMLayer::Walking => -1,
    };

    for ((_, from, to), weight) in edge_data {
        let from_cell = H3Cell {
            cell: from,
            layer: layer_id,
        };
        let to_cell = H3Cell {
            cell: to,
            layer: layer_id,
        };
        // connect the two cells in both directions
        graph.build_and_add_egde(from_cell, to_cell, Some(weight), None)?;
        graph.build_and_add_egde(to_cell, from_cell, Some(weight), None)?;

        if layer == OSMLayer::Cycling {
            // connect to the base layer
            let from_base_cell = H3Cell {
                cell: from,
                layer: -1,
            };
            let to_base_cell = H3Cell {
                cell: to,
                layer: -1,
            };
            graph.build_and_add_egde(from_cell, from_base_cell, Some(1.0), None)?;
            graph.build_and_add_egde(to_cell, to_base_cell, Some(1.0), None)?;
            graph.build_and_add_egde(from_base_cell, from_cell, Some(1.0), None)?;
            graph.build_and_add_egde(to_base_cell, to_cell, Some(1.0), None)?;
        }
    }
    Ok(graph)
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

    #[allow(clippy::type_complexity)]
    pub fn get_plot_data(&self) -> anyhow::Result<Vec<((f32, f32, f32), (f32, f32, f32))>> {
        let edges = &self.edges.as_ref().read().unwrap();
        let nodes = &self.nodes.as_ref().read().unwrap();
        let plot_data = edges
            .iter()
            .flat_map(|(key, edges)| {
                edges.iter().flat_map(move |edge| {
                    if let (Some(Some(start)), Some(Some(end))) =
                        (nodes.get(*key), nodes.get(edge.to))
                    {
                        let start_coords = h3o::LatLng::from(start.id.cell);
                        let start_plot = (
                            start_coords.lat() as f32,
                            start.id.layer as f32 / 100.0,
                            start_coords.lng() as f32,
                        );
                        let end_coords = h3o::LatLng::from(end.id.cell);
                        let end_plot = (
                            end_coords.lat() as f32,
                            end.id.layer as f32 / 100.0,
                            end_coords.lng() as f32,
                        );
                        Ok((start_plot, end_plot))
                    } else {
                        Err(anyhow::anyhow!("node not found"))
                    }
                })
            })
            .collect::<Vec<_>>();
        Ok(plot_data)
    }
}

#[pyclass]
pub struct PyH3Graph {
    graph: Graph<H3Cell>,
}

#[allow(unused_variables)]
#[pymethods]
impl PyH3Graph {
    #[new]
    pub fn new() -> Self {
        Self {
            graph: Graph::<H3Cell>::new(),
        }
    }

    pub fn create(&mut self, osm_path: &str, gtfs_path: &str) -> PyResult<()> {
        let start = Instant::now();
        let mut osm_graph =
            h3_network_from_osm(osm_path, OSMLayer::Walking).unwrap();
        println!(
            "osm graph created with {} nodes in {} s",
            osm_graph.nr_nodes(),
            start.elapsed().as_secs()
        );

        let start = Instant::now();
        let mut gtfs_graph = h3_network_from_gtfs(gtfs_path).unwrap();
        println!(
            "gtfs graph created with {} nodes in {} s",
            gtfs_graph.nr_nodes(),
            start.elapsed().as_secs()
        );

        let start = Instant::now();
        if osm_graph.merge(&mut gtfs_graph).is_ok() {
            self.graph = osm_graph;
            println!(
                "merged graph created with {} nodes in {} s",
                self.graph.nr_nodes(),
                start.elapsed().as_secs()
            );
        } else {
            println!("failed to merge graphs");
        }

        Ok(())
    }

    pub fn matrix_bfs(&self, origins: Vec<u64>, destinations: Vec<u64>) -> PyResult<Vec<Vec<f64>>> {
        // map each origin and destination to an H3 cell that is present in the graph
        let node_map_access = self.graph.node_map.as_ref().read().unwrap();

        let origins = u64list_to_cells(&node_map_access, origins);
        let destinations = u64list_to_cells(&node_map_access, destinations);

        let distances = self
            .graph
            .matrix_bfs_distance(origins, Some(destinations), false);

        Ok(distances
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|x| if let Some(x) = x { x } else { f64::MAX })
                    .collect()
            })
            .collect())
    }
}

fn u64list_to_cells(
    node_access: &RwLockReadGuard<BiHashMap<H3Cell, usize>>,
    list: Vec<u64>,
) -> Vec<H3Cell> {
    list.into_iter()
        .filter_map(|origin| {
            let cell_index: h3o::CellIndex = origin.try_into().ok()?;
            let cell = H3Cell {
                cell: cell_index,
                layer: -1,
            };
            if node_access.contains_left(&cell) {
                Some(cell)
            } else {
                let neighbors = cell_index.grid_ring_fast(1);
                for neighbor in neighbors.flatten() {
                    let neighbor_cell = H3Cell {
                        cell: neighbor,
                        layer: -1,
                    };
                    if node_access.contains_left(&neighbor_cell) {
                        return Some(neighbor_cell)
                    }
                }
                None
            }
        })
        .collect::<Vec<_>>()
}

impl Default for PyH3Graph {
    fn default() -> Self {
        Self::new()
    }
}
