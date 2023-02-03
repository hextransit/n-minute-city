use h3o::CellIndex;
use rayon::prelude::*;

pub fn process_gtfs(url: &str, h3_resolution: h3o::Resolution) -> anyhow::Result<()> {
    // let gtfs_url = "https://www.rejseplanen.info/labs/GTFS.zip";

    let feed = gtfs_structures::Gtfs::new(url)?;
    let trips = feed.trips;

    // for each trip, get stop sequence and travel times between stops
    let edge_data = trips
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
                    (route_id.clone(), start.0, end.0, weight)
                })
                .collect::<Vec<_>>();

            edges
        })
        .collect::<Vec<_>>();

    Ok(())
}
