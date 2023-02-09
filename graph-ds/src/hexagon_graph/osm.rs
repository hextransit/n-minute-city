use std::{collections::HashSet};

use h3o::CellIndex;
use osmpbf::{ElementReader, Element};


pub fn process_osm_pbf(url: &str) -> anyhow::Result<()> {
    let reader = ElementReader::from_path(url)?;
    let cells = reader.par_map_reduce(|element| {
        match element {
            Element::Way(way) => {
                if way.tags().any(|(k, v)| tag_value_matches(k, v)) {
                    let points = way.node_locations().map(|node| {
                        h3o::LatLng::new(node.lat(), node.lon()).unwrap().to_cell(h3o::Resolution::Twelve)
                    }).collect::<Vec<_>>();
                    println!("points: {:#?}", points.len());
                    points
                } else {
                    vec![]
                }
            }
            _ => {
                vec![]
            }
        }
    }, Vec::new, |a, b| {
        a.into_iter().chain(b.into_iter()).collect()
    })?.into_iter().collect::<HashSet<CellIndex>>();

    println!("cells: {:#?}", cells.len());
    Ok(())
}

pub fn tag_value_matches(tag: &str, value: &str) -> bool {
    match tag {
        "highway" => {
            !matches!(value, "motorway" | "motorway_link" | "prohibited" | "trunk" | "trunk_link" | "construction")
        },
        "access" => {
            !matches!(value, "private" | "no")
        },
        "foot" => {
            !matches!(value, "private" | "no")
        },
        _ => false
    }
}