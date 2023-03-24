use graph_ds::hexagon_graph::{h3_network_from_gtfs, h3_network_from_osm, OSMOptions};
use plotters::{prelude::*, style::full_palette::{ORANGE, LIGHTBLUE}};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let mut osm_graph = h3_network_from_osm("resources/denmark-processed.osm.pbf", &OSMOptions::default()).unwrap();

    // let mut cycle_graph = h3_network_from_osm(
    //     "resources/copenhagen-processed.osm.pbf",
    //     OSMLayer::Cycling,
    // ).unwrap();

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
    // osm_graph.merge(&mut cycle_graph)?;
    osm_graph.merge(&mut gtfs_graph)?;

    println!(
        "merged graph created with {} nodes in {} s",
        osm_graph.nr_nodes(),
        start.elapsed().as_secs()
    );

    plot_png(osm_graph.get_plot_data().unwrap());

    Ok(())
}

#[allow(clippy::type_complexity)]
fn plot_png(
    plot_data: Vec<((f32, f32, f32), (f32, f32, f32))>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "plotting {} edges, e.g. {:?}",
        plot_data.len(),
        plot_data.first()
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

    let (y_min, y_max) = (-3_f32, 7_f32);

    // let (x_min, x_max, y_min, y_max, z_min, z_max) =
    //     (55_f32, 56_f32, -3_f32, 7_f32, 12_f32, 13_f32);

    println!("x: {} .. {}", x_min, x_max);
    println!("y: {} .. {}", y_min, y_max);
    println!("z: {} .. {}", z_min, z_max);

    let root = plotters::backend::BitMapBackend::new("test.png", (16000, 16000)).into_drawing_area();

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(0)
        .y_label_area_size(0)
        .build_cartesian_3d(x_min..x_max, y_min..y_max, z_min..z_max)?;

    chart.with_projection(|mut pb| {
        pb.pitch = 0.5;
        pb.yaw = 0.5;
        pb.scale = 1.0;
        pb.into_matrix()
    });

    chart.draw_series(plot_data.into_iter().map(|data| {
        match (data.0 .1 as i32, data.1 .1 as i32) {
            (0, 0) => PathElement::new(vec![data.0, data.1], BLUE.mix(0.5)),
            (-1, -1) => PathElement::new(vec![data.0, data.1], ORANGE.mix(0.5)),
            (-2, -2) => PathElement::new(vec![data.0, data.1], RED.mix(0.5)),
            (-1, 0) => PathElement::new(vec![data.0, data.1], LIGHTBLUE.mix(0.2)),
            _ => PathElement::new(vec![data.0, data.1], YELLOW.mix(0.01)),
        }
    }))?;

    root.present()?;
    Ok(())
}
