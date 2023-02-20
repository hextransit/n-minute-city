use std::collections::HashMap;

use h3o::CellIndex;
use rayon::prelude::*;

// TODO: calculate frequencies at each stop
#[allow(clippy::type_complexity)]
pub fn process_gtfs(
    url: &str,
    h3_resolution: h3o::Resolution,
) -> anyhow::Result<Vec<((usize, CellIndex, CellIndex), f64)>> {
    // let gtfs_url = "https://www.rejseplanen.info/labs/GTFS.zip";

    println!("getting GTFS feed from {url}");

    let feed = gtfs_structures::GtfsReader::default()
        .trim_fields(false)
        .read(url)?;

    // let trips = feed.trips;
    let route_data: HashMap<String, usize> = feed
        .routes
        .keys()
        .enumerate()
        .par_bridge()
        .map(|(index, route)| (route.clone(), index))
        .collect();

    println!("routes: {}", route_data.len());

    // for each trip, get stop sequence and travel times between stops
    // collects a vec of unique edges, identified by (route_id, start, end) and weight = travel time
    let edge_data = feed
        .trips
        .into_values()
        .par_bridge()
        .flat_map(|trip| {
            let route_id = trip.route_id;

            let mut stop_sequence: Vec<(CellIndex, u16, Option<u32>, Option<u32>)> = trip
                .stop_times
                .par_iter()
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
            // let mut stop_frequencies: HashMap<CellIndex, Vec<time::Time>> = HashMap::new();

            let edges = stop_sequence
                .windows(2)
                .filter_map(|window| {
                    let (start, end) = (window[0], window[1]);
                    if let (Ok(start_time), Ok(end_time)) = (
                        convert_gtfs_time(start.2.unwrap()),
                        convert_gtfs_time(end.3.unwrap()),
                    ) {
                        if end_time > start_time {
                            let duration = (end_time - start_time).whole_minutes();
                            // stop_frequencies
                            //     .entry(start.0)
                            //     .and_modify(|f| f.push(start_time))
                            //     .or_insert_with(|| vec![start_time]);
                            Some((
                                (*route_data.get(&route_id).unwrap_or(&0), start.0, end.0),
                                duration as f64,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<HashMap<_, _>>();

            edges
        })
        .collect::<Vec<_>>();

    Ok(edge_data)
}

// reverse operation of time = hours * 3600 + minutes * 60 + seconds
fn convert_gtfs_time(time: u32) -> anyhow::Result<time::Time> {
    let hours = time / 3600;
    let minutes = (time - hours * 3600) / 60;
    let seconds = time - hours * 3600 - minutes * 60;
    Ok(time::Time::from_hms(
        hours as u8,
        minutes as u8,
        seconds as u8,
    )?)
}
