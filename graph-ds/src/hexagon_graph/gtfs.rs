use std::collections::{BTreeMap, HashMap};

use h3o::CellIndex;
use rayon::prelude::*;

use itertools::Itertools;

#[allow(clippy::type_complexity)]
pub struct GtfsProcessingResult {
    pub edge_data: Vec<((usize, CellIndex, CellIndex), f64)>,
    pub stop_frequencies: HashMap<(CellIndex, usize), Vec<f64>>,
}

/// calculates the frequencies at each stop for every route
/// 
/// frequencies are stored per hour of the week, the frequency number is the number of departures per hour
pub fn calcualte_stop_frequencies(
    trips: &HashMap<String, gtfs_structures::Trip>, 
    calendar: &HashMap<String, gtfs_structures::Calendar>,
    route_layer_map: HashMap<String, usize>,
    h3_resolution: h3o::Resolution,
) -> anyhow::Result<HashMap<(CellIndex, usize), Vec<f64>>> {
    let stop_times = trips.values().flat_map(|trip| {
        let test = trip.stop_times.iter().flat_map(|stop_time| {
            let stop = &stop_time.stop;
            if let (Some(lat), Some(lon), Some(departure_time), Some(route_id)) = (stop.latitude, stop.longitude, stop_time.departure_time, route_layer_map.get(&trip.route_id)) {
                if let (Ok(departure_time), Some(calendar)) = (convert_gtfs_time(departure_time), calendar.get(&trip.service_id)) {
                    let h3 = h3o::LatLng::new(lat, lon).unwrap().to_cell(h3_resolution);
                    let days = [calendar.monday, calendar.tuesday, calendar.wednesday, calendar.thursday, calendar.friday, calendar.saturday, calendar.sunday];
                    days.iter().enumerate().filter_map(|(day_idx, service_is_running)| {
                        if *service_is_running {
                            Some(((h3, *route_id), (departure_time, day_idx)))
                        } else {
                            None
                        }
                    }).collect::<Vec<((CellIndex, usize), (time::Time, usize))>>()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }).collect::<Vec<((CellIndex, usize), (time::Time, usize))>>();
        test
    }).into_group_map().par_iter().map(
        |((h3, route_id), times)| {
            let mut frequencies = vec![0.0; 24 * 7];
            for ((day, hour), group) in times.iter().into_group_map_by(|(time, day)| (day, time.hour())) {
                let count = group.len() as f64;
                frequencies[hour as usize + day * 24] = count;
            }
            ((*h3, *route_id), frequencies)
        }
    ).collect::<HashMap<_, _>>();
    
    Ok(stop_times)

}

/// calculate the time it takes to travel between any two stops on a route
/// 
/// returns a vecor containing edge data, where each element is a tuple of (route_id, start_stop, end_stop) and the duration between them
#[allow(clippy::type_complexity)]
pub fn calculate_edge_data(
    trips: &HashMap<String, gtfs_structures::Trip>,
    route_layer_map: HashMap<String, usize>, 
    h3_resolution: h3o::Resolution
) -> anyhow::Result<Vec<((usize, CellIndex, CellIndex), f64)>> {
    let edge_data = trips
    .values()
    .par_bridge()
    .flat_map(|trip| {
        let route_id = &trip.route_id;

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
                        Some((
                            (*route_layer_map.get(route_id).unwrap_or(&0), start.0, end.0),
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

/// process the GTFS feed and return both the edge data and the stop frequencies
pub fn process_gtfs(
    url: &str,
    h3_resolution: h3o::Resolution,
) -> anyhow::Result<GtfsProcessingResult> {
    // let gtfs_url = "https://www.rejseplanen.info/labs/GTFS.zip";

    println!("getting GTFS feed from {url}");

    let feed = gtfs_structures::GtfsReader::default()
        .trim_fields(false)
        .read(url)?;

    let route_data: HashMap<String, usize> = feed
        .routes
        .iter()
        .collect::<BTreeMap<_, _>>()
        .keys()
        .enumerate()
        .par_bridge()
        .map(|(index, route)| (route.to_string(), index))
        .collect();

    println!("routes: {}", route_data.len());

    Ok(GtfsProcessingResult {
        edge_data: calculate_edge_data(&feed.trips, route_data.clone(), h3_resolution)?,
        stop_frequencies: calcualte_stop_frequencies(&feed.trips, &feed.calendar, route_data, h3_resolution)?,
    })
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
