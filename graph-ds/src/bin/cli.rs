use graph_ds::{
    hexagon_graph::{h3_network_from_osm, osm::OSMLayer, h3_network_from_gtfs},
};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let mut osm_graph = h3_network_from_osm("resources/copenhagen-with-ways.osm.pbf", OSMLayer::Walking).unwrap();
    println!("osm graph created with {} nodes in {} s", osm_graph.nr_nodes(), start.elapsed().as_secs());

    let start = Instant::now();
    let mut gtfs_graph = h3_network_from_gtfs("resources/rejseplanen.zip").unwrap();
    println!("gtfs graph created with {} nodes in {} s", gtfs_graph.nr_nodes(), start.elapsed().as_secs());

    let start = Instant::now();
    osm_graph.merge(&mut gtfs_graph)?;

    println!("merged graph created with {} nodes in {} s", osm_graph.nr_nodes(), start.elapsed().as_secs());

    Ok(())

    // let graph: Graph<Cell> = hexagon_graph_from_file("graph-ds/de_inferno_hexagons.mpk")?;

    // println!("ready");

    // let start = Cell {
    //     a: 60,
    //     b: -33,
    //     radius: 24,
    //     layer: 3,
    // };
    // let end = Cell {
    //     a: 5,
    //     b: 61,
    //     radius: 24,
    //     layer: 3,
    // };

    // println!("start: {start:?}, end: {end:?}");
    // let now = Instant::now();
    // let (Some(path), _) = graph.bfs(start, Some(end))? else {
    //     println!("backtracing failed");
    //     return Ok(());
    // };
    // println!("time: {:?} µs", now.elapsed().as_micros());
    // println!("path: {:?} ({})", path, path.len());

    // println!("matrix bfs");
    // let iterations = 10000;
    // let now = Instant::now();
    // graph.matrix_bfs_distance(vec![start; iterations], true);
    // let elapsed = now.elapsed().as_micros();
    // println!(
    //     "time: {:?} µs ({:?} µs /iteration)",
    //     elapsed,
    //     elapsed / iterations as u128
    // );

    // Ok(())
}
