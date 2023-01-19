use graph_ds::{Graph, hexagon_graph::{cell::Cell, hexagon_graph_from_file}};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let graph : Graph<Cell> = hexagon_graph_from_file("graph-ds/de_inferno_hexagons.mpk")?;

    let start = graph.nodes.get(0).unwrap().as_ref().unwrap().id;
    let end = graph.nodes.get(4).unwrap().as_ref().unwrap().id;

    println!("start: {:?}, end: {:?}", start, end);
    let now = Instant::now();
    let (_, distances) = graph.bfs(start, Some(end))?;
    println!("distance: {:?}", distances.get(&end).unwrap());
    println!("time: {:?} µs", now.elapsed().as_micros());

    Ok(())
}