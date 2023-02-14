use h3o::CellIndex;
use osmpbf::{Element, ElementReader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OSMLayer {
    Cycling,
    Walking,
}

/// converts a OSM pbf file into a hexagonal graph layer
/// * Cycling: 4.5 m/s, layer_idx: 0
/// * Walking: 1.4 m/s, layer_idx: 1
#[allow(clippy::type_complexity)]
pub fn process_osm_pbf(
    url: &str,
    layer: OSMLayer,
    h3_resolution: h3o::Resolution,
) -> anyhow::Result<Vec<((OSMLayer, CellIndex, CellIndex), f64)>> {
    let reader = ElementReader::from_path(url)?;
    let cell_distance = h3_resolution.edge_length_m();

    let transition_weight = match layer {
        OSMLayer::Cycling => cell_distance / 4.5 / 60.0,
        OSMLayer::Walking => cell_distance / 1.4 / 60.0,
    };

    println!("processing osm pbf file: {url}");

    let edge_data = reader
        .par_map_reduce(
            |element| {
                match element {
                    Element::Way(way) => {
                        if way.tags().any(|(k, v)| tag_value_matches(k, v, &layer)) {
                            // let node_refs = way.refs().collect::<Vec<_>>();
                            let node_points = way
                                .node_locations()
                                .map(|node| {
                                    h3o::LatLng::new(node.lat(), node.lon())
                                        .unwrap()
                                        .to_cell(h3_resolution)
                                })
                                .collect::<Vec<_>>();
                            // for each pair of points, add the points in between
                            let path_points = node_points
                                .windows(2)
                                .flat_map(|cells| {
                                    let a = cells[0];
                                    let b = cells[1];
                                    if let Ok(path) = a.grid_path_cells(b) {
                                        path.into_iter().flatten().collect()
                                    } else {
                                        vec![]
                                    }
                                })
                                .collect::<Vec<CellIndex>>();
                            //TODO: maybe connect all edges, not just one?
                            let edges = path_points
                                .windows(2)
                                .flat_map(|cells| {
                                    let a = cells[0];
                                    let b = cells[1];
                                    if a != b {
                                        Ok(((layer, a, b), transition_weight))
                                    } else {
                                        Err(anyhow::anyhow!("same cell"))
                                    }
                                })
                                .collect::<Vec<((OSMLayer, CellIndex, CellIndex), f64)>>();
                            edges
                        } else {
                            vec![]
                        }
                    }
                    _ => {
                        vec![]
                    }
                }
            },
            Vec::new,
            |a, b| a.into_iter().chain(b.into_iter()).collect(),
        )?
        .into_iter()
        .collect::<Vec<_>>();

    println!("converted OSM file into {:#?} edges", edge_data.len());

    Ok(edge_data)
}

pub fn tag_value_matches(tag: &str, value: &str, layer: &OSMLayer) -> bool {
    match tag {
        "highway" => !matches!(
            value,
            "motorway" | "motorway_link" | "prohibited" | "trunk" | "trunk_link" | "construction"
        ),
        "access" => !matches!(value, "private" | "no"),
        "foot" => {
            if layer == &OSMLayer::Walking {
                !matches!(value, "private" | "no")
            } else {
                false
            }
        }
        "bicycle" => {
            if layer == &OSMLayer::Cycling {
                !matches!(value, "private" | "no" | "none")
            } else {
                false
            }
        }
        "cycleway" => {
            if layer == &OSMLayer::Cycling {
                !matches!(value, "shared" | "no" | "none")
            } else {
                false
            }
        }
        "bycicle_road" => true,
        "bycicle" => true,
        _ => false,
    }
}
