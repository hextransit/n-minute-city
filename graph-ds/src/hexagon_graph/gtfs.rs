use std::collections::HashMap;

use h3o::CellIndex;
use rayon::prelude::*;

#[allow(clippy::type_complexity)]
pub fn process_gtfs(url: &str, h3_resolution: h3o::Resolution) -> anyhow::Result<Vec<((usize, CellIndex, CellIndex), f64)>> {
    // let gtfs_url = "https://www.rejseplanen.info/labs/GTFS.zip";

    let feed = gtfs_structures::Gtfs::new(url)?;
    // let trips = feed.trips;

    let route_data: HashMap<String, usize> = feed.routes.keys().enumerate().par_bridge().map(|(index, route)| {
        (route.clone(), index)
    }).collect();

    // for each trip, get stop sequence and travel times between stops
    // collects a vec of unique edges, identified by (route_id, start, end) and weight = travel time
    let edge_data = feed.trips
        .into_values()
        .par_bridge()
        .flat_map(|trip| {
            let route_id = trip.route_id;

            let mut stop_sequence: Vec<(CellIndex, u16, Option<u32>, Option<u32>)> = trip
                .stop_times
                .iter()
                .par_bridge()
                .filter_map(|stop_time| {
                    let seq = stop_time.stop_sequence;
                    let arrival = stop_time.arrival_time;
                    let departure = stop_time.departure_time;
                    let stop = &stop_time.stop;
                    if let (Some(lat), Some(lon)) = (stop.latitude, stop.longitude) {
                        let h3 = h3o::LatLng::new(lat, lon).unwrap().to_cell(h3_resolution);
                        Some((h3, seq, arrival, departure))
                    } else {
                        None
                    }
                })
                .collect();

            stop_sequence.sort_unstable_by_key(|x| x.1);

            let edges = stop_sequence
                .windows(2)
                .map(|window| {
                    let (start, end) = (window[0], window[1]);
                    let weight = (end.3.unwrap() - start.2.unwrap()) as f64;
                    ((*route_data.get(&route_id).unwrap_or(&0), start.0, end.0), weight)
                })
                .collect::<HashMap<_, _>>();

            edges
        })
        .collect::<Vec<_>>();

    Ok(edge_data)
}
