use graph_ds::hexagon_graph::{h3_network_from_gtfs, h3_network_from_osm, osm::OSMLayer};
use plotters::prelude::*;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let mut osm_graph =
        h3_network_from_osm("resources/denmark-with-ways.osm.pbf", OSMLayer::Walking).unwrap();
    println!(
        "osm graph created with {} nodes in {} s",
        osm_graph.nr_nodes(),
        start.elapsed().as_secs()
    );

    let start = Instant::now();
    let mut gtfs_graph = h3_network_from_gtfs("resources/rejseplanen.zip").unwrap();
    println!(
        "gtfs graph created with {} nodes in {} s",
        gtfs_graph.nr_nodes(),
        start.elapsed().as_secs()
    );

    let start = Instant::now();
    osm_graph.merge(&mut gtfs_graph)?;

    println!(
        "merged graph created with {} nodes in {} s",
        osm_graph.nr_nodes(),
        start.elapsed().as_secs()
    );

    plot_png(osm_graph.get_plot_data().unwrap());

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

fn plot_png(
    plot_data: Vec<((f32, f32, f32), (f32, f32, f32))>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "plotting {} edges, e.g. {:?}",
        plot_data.len(),
        plot_data.get(0)
    );

    let (x_min, x_max, y_min, y_max, z_min, z_max) = plot_data.iter().fold(
        (f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MAX, f32::MIN),
        |(x_min, x_max, y_min, y_max, z_min, z_max), ((x1, y1, z1), (x2, y2, z2))| {
            (
                x_min.min(*x1).min(*x2),
                x_max.max(*x1).max(*x2),
                y_min.min(*y1).min(*y2),
                y_max.max(*y1).max(*y2),
                z_min.min(*z1).min(*z2),
                z_max.max(*z1).max(*z2),
            )
        },
    );

    println!("x: {} .. {}", x_min, x_max);
    println!("y: {} .. {}", y_min, y_max);
    println!("z: {} .. {}", z_min, z_max);

    let root = plotters::backend::BitMapBackend::new("test.png", (4096, 4096)).into_drawing_area();

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(0)
        .y_label_area_size(0)
        .build_cartesian_3d(x_min..x_max, y_min..y_max, z_min..z_max)?;

    chart.with_projection(|mut pb| {
        pb.pitch = 0.2;
        pb.yaw = 0.2;
        pb.scale = 1.2;
        pb.into_matrix()
    });

    chart.draw_series(plot_data.into_iter().map(|data| {
        if data.0 .1 != data.1 .1 {
            PathElement::new(vec![data.0, data.1], BLUE.mix(0.01))
        } else {
            PathElement::new(vec![data.0, data.1], RED.mix(0.5))
        }
    }))?;

    root.present()?;
    Ok(())
}
