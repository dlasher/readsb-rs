use crate::tracking::{Aircraft, haversine_distance};
use crate::viewsb::cli::SortColumn;
pub fn format_row(_a: &Aircraft, _metric: bool, _show_dist: bool, _dist_nm: f64, _seen_secs: i64) -> String {
    String::new()
}

pub fn format_header(_metric: bool, _show_dist: bool) -> String {
    String::new()
}

pub fn format_separator(_metric: bool, _show_dist: bool) -> String {
    String::new()
}

pub fn format_aircraft_table(
    _aircraft: &[Aircraft],
    _now: i64,
    _metric: bool,
    _show_dist: bool,
    _user_lat: f64,
    _user_lon: f64,
) -> Vec<String> {
    vec![]
}

pub fn compute_distance_nm(a: &Aircraft, user_lat: f64, user_lon: f64) -> f64 {
    if a.lat == 0.0 && a.lon == 0.0 {
        return 0.0;
    }
    let meters = haversine_distance(user_lat, user_lon, a.lat, a.lon);
    meters / 1852.0
}

pub fn compute_distance_km(a: &Aircraft, user_lat: f64, user_lon: f64) -> f64 {
    if a.lat == 0.0 && a.lon == 0.0 {
        return 0.0;
    }
    let meters = haversine_distance(user_lat, user_lon, a.lat, a.lon);
    meters / 1000.0
}

pub fn sort_aircraft(_aircraft: &mut Vec<Aircraft>, _sort: SortColumn, _user_lat: f64, _user_lon: f64) {
}
