use graph_ds::{
    hexagon_graph::{cell::Cell, hexagon_graph_from_file},
    Graph,
};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let graph: Graph<Cell> = hexagon_graph_from_file("graph-ds/de_inferno_hexagons.mpk")?;

    println!("ready");

    let start = Cell {
        a: 60,
        b: -33,
        radius: 24,
        layer: 3,
    };
    let end = Cell {
        a: 5,
        b: 61,
        radius: 24,
        layer: 3,
    };

    println!("start: {:?}, end: {:?}", start, end);
    let now = Instant::now();
    let (Some(path), _) = graph.bfs(start, Some(end))? else {
        println!("backtracing failed");
        return Ok(());
    };
    println!("time: {:?} µs", now.elapsed().as_micros());
    println!("path: {:?} ({})", path, path.len());

    println!("matrix bfs");
    let iterations = 10000;
    let now = Instant::now();
    graph.matrix_bfs_distance(vec![start; iterations], None, true);
    let elapsed = now.elapsed().as_micros();
    println!("time: {:?} µs ({:?} µs /iteration)", elapsed, elapsed / iterations as u128);

    Ok(())
}
