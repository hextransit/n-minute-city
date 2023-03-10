pub mod cell;
pub mod gtfs;
pub mod h3cell;
pub mod osm;

use std::{collections::HashMap, sync::RwLockReadGuard, time::Instant};

use crate::{Edge, Graph};
use bimap::BiHashMap;
use cell::Cell;
use rayon::prelude::*;

use self::{
    cell::Direction,
    h3cell::H3Cell,
    osm::{process_osm_pbf, OSMLayer},
};

pub struct OSMOptions {
    layer: Option<OSMLayer>,
    bike_penalty: f64
}

impl Default for OSMOptions {
    fn default() -> Self {
        OSMOptions {
            layer: None,
            bike_penalty: 1.0
        }
    }
}

#[cfg(feature = "pyo3")]
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
            self.build_and_add_egde(cell, neighbor, weight, None, None)?
        }

        Ok(())
    }
}

pub fn cell_graph_from_mpk(path: &str) -> anyhow::Result<Graph<Cell>> {
    //reader
    let file = std::fs::File::open(path)?;
    let mut graph = Graph::<Cell>::new();
    // let edges: HashSet<(Cell, Cell)> = rmp_serde::from_read(file)?;
    let edges: Vec<(Cell, Cell, f32)> = rmp_serde::from_read(file)?;

    edges.iter().for_each(|(from, to, weight)| {
        let res = graph.build_and_add_egde(*from, *to, Some(*weight as f64), None, None);
        if res.is_err() {
            println!("error: {res:?}");
        }
    });
    Ok(graph)
}

pub fn h3_network_from_osm(
    osm_url: &str,
    options: &OSMOptions,
) -> anyhow::Result<Graph<H3Cell>> {
    let edge_data = process_osm_pbf(osm_url, options, h3o::Resolution::Twelve)?;

    let mut graph = Graph::<H3Cell>::new();

    for ((layer, from, to), weight) in edge_data {
        let from_cell = H3Cell {
            cell: from,
            layer: layer.get_id(),
        };
        let to_cell = H3Cell {
            cell: to,
            layer: layer.get_id(),
        };
        // connect the two cells in both directions
        graph.build_and_add_egde(from_cell, to_cell, Some(weight), None, None)?;
        graph.build_and_add_egde(to_cell, from_cell, Some(weight), None, None)?;

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
            graph.build_and_add_egde(from_cell, from_base_cell, Some(options.bike_penalty), None, None)?;
            graph.build_and_add_egde(to_cell, to_base_cell, Some(options.bike_penalty), None, None)?;
            graph.build_and_add_egde(from_base_cell, from_cell, Some(options.bike_penalty), None, None)?;
            graph.build_and_add_egde(to_base_cell, to_cell, Some(options.bike_penalty), None, None)?;
        }
    }
    Ok(graph)
}

pub fn h3_network_from_gtfs(gtfs_url: &str) -> anyhow::Result<Graph<H3Cell>> {
    let gtfs_res = gtfs::process_gtfs(gtfs_url, h3o::Resolution::Twelve)?;
    let weight_lists = gtfs_res.stop_frequencies;
    let mut graph = Graph::<H3Cell>::new();
    for ((layer, from, to), weight) in gtfs_res.edge_data {
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
        if let Some(weight_list) = weight_lists.get(&(from, layer)) {
            // connect from base layer with weight list
            if weight_list.len() != 24 * 7 {
                graph.build_and_add_egde(base_cell, from_cell, Some(5.0), None, None)?;
            } else {
                let list_average = 60.0
                    / (weight_list.iter().filter(|x| !x.is_infinite()).sum::<f64>()
                        / weight_list.len() as f64)
                    / 2.0;
                let weight_list = weight_list
                    .iter()
                    .map(|x| 60.0 / x / 2.0)
                    .collect::<Vec<_>>();
                graph.build_and_add_egde(
                    base_cell,
                    from_cell,
                    Some(list_average),
                    Some(weight_list),
                    None,
                )?;
            }
        } else {
            // connect from base layer with weight 5
            graph.build_and_add_egde(base_cell, from_cell, Some(5.0), None, None)?;
        }
        // the transit edge
        graph.build_and_add_egde(from_cell, to_cell, Some(weight), None, None)?;
        // the connection from the transit edge to the base layer
        graph.build_and_add_egde(from_cell, base_cell, Some(1.0), None, None)?;
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
                        let start_layer = if start.id.layer >= 0 {
                            0.0
                        } else {
                            start.id.layer as f32
                        };
                        let start_coords = h3o::LatLng::from(start.id.cell);
                        let start_plot = (
                            start_coords.lat() as f32,
                            start_layer,
                            start_coords.lng() as f32,
                        );
                        let end_layer = if end.id.layer >= 0 {
                            0.0
                        } else {
                            end.id.layer as f32
                        };
                        let end_coords = h3o::LatLng::from(end.id.cell);
                        let end_plot =
                            (end_coords.lat() as f32, end_layer, end_coords.lng() as f32);
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

#[cfg(feature = "pyo3")]
#[pyclass]
pub struct PyH3Graph {
    graph: Graph<H3Cell>,
    options: OSMOptions, 
    k_ring: u32,
}

#[cfg(feature = "pyo3")]
#[allow(unused_variables)]
#[pymethods]
impl PyH3Graph {
    #[new]
    pub fn new(bike_penalty: f64, k_ring: u32) -> Self {
        Self {
            graph: Graph::<H3Cell>::new(),            
            options: OSMOptions {
                layer: None,
                bike_penalty,
            },
            k_ring,
        }
    }

    pub fn create(&mut self, osm_path: &str, gtfs_path: &str) -> PyResult<()> {
        let start = Instant::now();
        let mut osm_graph = h3_network_from_osm(osm_path, &self.options).unwrap();

        println!(
            "osm graph created with ({}) nodes (walk + bike) in {} s",
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
            return Err(PyErr::new::<pyo3::exceptions::PyException, _>(
                "failed to merge graphs",
            ));
        }

        println!("hash: {}", self.graph.node_hash());

        Ok(())
    }

    pub fn astar_path(&self, origin: u64, destination: u64) -> PyResult<(Vec<u64>, f64)> {
        fn h(start_cell: &H3Cell, end_cell: &H3Cell) -> f64 {
            if let Ok(dist) = start_cell.cell.grid_distance(end_cell.cell) {
                dist as f64
            } else {
                println!(
                    "grid distance failed between {} and {}",
                    start_cell.cell, end_cell.cell
                );
                i32::MAX as f64
            }
        }

        let node_map_access = self.graph.node_map.as_ref().read().unwrap();
        let node_mapping = u64list_to_h3cells(&node_map_access, vec![origin, destination], self.k_ring);

        node_mapping.iter().for_each(|(original, mapped)| {
            if let Some(mapped) = mapped {
                let mapped_u64 = u64::from(mapped.cell);
                if original != &mapped_u64 {
                    println!("nodes have been adjusted: {} -> {}", original, mapped_u64);
                }
            }
        });

        let (Some(Some(origin)), Some(Some(destination))) = (node_mapping.get_by_left(&origin), node_mapping.get_by_left(&destination)) else {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "origin or destination not found",
            ));
        };

        println!(
            "astar from {} to {}",
            u64::from(origin.cell),
            u64::from(destination.cell)
        );

        let astar_res = self
            .graph
            .astar(*origin, Some(*destination), &None, None, h);

        if let Ok(astar_res) = astar_res {
            if let (Some(path), Some(distance)) = (astar_res.path, astar_res.distances.first()) {
                let u64_path = path
                    .into_iter()
                    .flat_map(|cell| cell.cell.try_into())
                    .collect::<Vec<u64>>();

                Ok((u64_path, *distance))
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "no path found",
                ))
            }
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "no path found",
            ))
        }
    }

    pub fn matrix_distance(
        &self,
        origins: Vec<u64>,
        destinations: Vec<u64>,
        hour_of_week: Option<usize>,
    ) -> PyResult<HashMap<u64, Vec<f64>>> {
        fn h(start_cell: &H3Cell, end_cell: &H3Cell) -> f64 {
            start_cell
                .cell
                .grid_distance(end_cell.cell)
                .unwrap_or(i32::MAX) as f64
        }

        // map each origin and destination to an H3 cell that is present in the graph
        let node_map_access = self.graph.node_map.as_ref().read().unwrap();

        let origins = u64list_to_h3cells(&node_map_access, origins, self.k_ring);
        let destinations = u64list_to_h3cells(&node_map_access, destinations, self.k_ring);

        let distances = self.graph.matrix_astar_distance(
            origins.iter().filter_map(|(_, c)| *c).collect::<Vec<_>>(),
            Some(
                destinations
                    .iter()
                    .filter_map(|(_, c)| *c)
                    .collect::<Vec<_>>(),
            ),
            false,
            hour_of_week,
            h,
        );

        println!("matrix distance computed for {} origins - got {} results", origins.len(), distances.len());

        Ok(distances
            .into_par_iter()
            .map(|(graph_origin, distances)| {
                let original_origin: u64 = *origins.get_by_right(&Some(graph_origin)).unwrap();
                if let Ok(row) = distances {
                    (original_origin, row)
                } else {
                    (original_origin, vec![])
                }
            })
            .collect())
    }
}

/// returns processed H3 cells in a list of tuples (original H3 input, mapped H3 cell)
///
/// H3 cells that are not present in the graph are mapped to their first neighbor that is present in the graph
/// or none if no cells can be found within a k-ring of size 2
fn u64list_to_h3cells(
    node_access: &RwLockReadGuard<BiHashMap<H3Cell, usize>>,
    list: Vec<u64>,
    k_ring: u32,
) -> BiHashMap<u64, Option<H3Cell>> {
    list.into_iter()
        .filter_map(|origin| {
            let cell_index: h3o::CellIndex = origin.try_into().ok()?;
            let cell = H3Cell {
                cell: cell_index,
                layer: -1,
            };
            if node_access.contains_left(&cell) {
                Some((origin, Some(cell)))
            } else {
                let neighbors = cell_index.grid_ring_fast(k_ring);
                for neighbor in neighbors.flatten() {
                    let neighbor_cell = H3Cell {
                        cell: neighbor,
                        layer: -1,
                    };
                    if node_access.contains_left(&neighbor_cell) {
                        return Some((origin, Some(neighbor_cell)));
                    }
                }
                Some((origin, None))
            }
        })
        .collect::<BiHashMap<_, _>>()
}

#[cfg(feature = "pyo3")]
impl Default for PyH3Graph {
    fn default() -> Self {
        Self::new(1.0, 2)
    }
}

#[cfg(feature = "pyo3")]
#[pyclass]
pub struct PyCellGraph {
    graph: Graph<Cell>,
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl PyCellGraph {
    #[new]
    pub fn new() -> Self {
        Self {
            graph: Graph::<Cell>::new(),
        }
    }

    pub fn create_from_mpk(&mut self, mpk_path: &str) -> PyResult<()> {
        let start = Instant::now();
        let mpk_graph = cell_graph_from_mpk(mpk_path).unwrap();
        println!(
            "mpk graph created with {} nodes in {} ms, hash: {}",
            mpk_graph.nr_nodes(),
            start.elapsed().as_millis(),
            mpk_graph.node_hash()
        );
        self.graph = mpk_graph;
        Ok(())
    }

    pub fn matrix_distance(
        &self,
        origins: Vec<u64>,
        destinations: Vec<u64>,
    ) -> PyResult<HashMap<u64, Vec<f64>>> {
        fn heuristic(start_cell: &Cell, end_cell: &Cell) -> f64 {
            let dx = (start_cell.a - end_cell.a).abs();
            let dy = (start_cell.b - end_cell.b).abs();
            let dz = (start_cell.a + start_cell.b - end_cell.a - end_cell.b).abs();
            let dlayer = (start_cell.layer - end_cell.layer).abs();
            ((dx + dy + dz) as f64 / 2.0 + dlayer as f64) * (start_cell.radius * 2) as f64
        }

        let origins = origins
            .iter()
            .map(|o| Cell::from_id(*o))
            .collect::<Vec<_>>();
        let destinations = destinations
            .iter()
            .map(|d| Cell::from_id(*d))
            .collect::<Vec<_>>();
        let distances =
            self.graph
                .matrix_astar_distance(origins, Some(destinations), false, None, heuristic);

        Ok(distances
            .into_par_iter()
            .map(|(start, row)| {
                if let Ok(row) = row {
                    (start.id(), row)
                } else {
                    (start.id(), vec![])
                }
            })
            .collect())
    }
}

#[cfg(feature = "pyo3")]
impl Default for PyCellGraph {
    fn default() -> Self {
        Self::new()
    }
}
