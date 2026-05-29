use crate::tracking::{Aircraft, haversine_distance};
use crate::types::altitude::INVALID_ALTITUDE;
use crate::viewsb::cli::SortColumn;

pub fn format_row(a: &Aircraft, metric: bool, show_dist: bool, dist_nm: f64, seen_secs: i64) -> String {
    let alt = if a.baro_alt != INVALID_ALTITUDE {
        if metric {
            (a.baro_alt as f64 * 0.3048).round() as i32
        } else {
            a.baro_alt
        }
    } else if a.geom_alt != INVALID_ALTITUDE {
        if metric {
            (a.geom_alt as f64 * 0.3048).round() as i32
        } else {
            a.geom_alt
        }
    } else {
        0
    };
    let spd = if a.gs > 0.0 {
        if metric {
            (a.gs as f64 * 1.852).round() as i32
        } else {
            a.gs as i32
        }
    } else {
        0
    };
    let dist_str = if show_dist {
        let d = if metric {
            dist_nm * 1.852
        } else {
            dist_nm
        };
        format!(" {:5.1}", d)
    } else {
        String::new()
    };
    let rssi = format!("{:5.1}", a.get_signal_db());
    let trk = format!("{:3.0}", a.track);
    let lat = format!("{:7.2}", a.lat);
    let lon = format!("{:8.2}", a.lon);
    let seen = format!("{:4}", seen_secs);
    let msgs = format!("{:4}", a.messages);
    let alt_str = format!("{:5}", alt);
    let spd_str = format!("{:4}", spd);

    format!(
        "{:06X}  {:<8} {:04x}  {:>5} {:>4}  {:>7} {:>8}  {:>3} {:>4} {:>4}  {:>5}{}",
        a.addr,
        a.callsign.as_str(),
        a.squawk,
        alt_str,
        spd_str,
        lat,
        lon,
        trk,
        msgs,
        seen,
        rssi,
        dist_str
    )
}

pub fn format_header(metric: bool, show_dist: bool) -> String {
    let alt_unit = if metric { "m " } else { "ft " };
    let spd_unit = if metric { "km/h" } else { " kt" };
    let dist_col = if show_dist { "  Dist" } else { "" };
    format!(
        "{:6}  {:<8} {:4}  {:>5} {:>4}  {:>7} {:>8}  {:>3} {:>4} {:>4}  {:>5}{}",
        "ICAO", "Callsign", "Sqwk", alt_unit, spd_unit, "Lat", "Lon", "Hdg", "Msgs", "Seen", "RSSI", dist_col
    )
}

pub fn format_separator(show_dist: bool) -> String {
    // Separator matches data format: dashes for each column width, spaces matching format gaps
    // {:06X}  {:<8} {:04o}  {:>5} {:>4}  {:>7} {:>8}  {:>3} {:>4} {:>4}  {:>5}
    //    6   2    8   1    4   2    5   1    4   2    7   1    8   2    3   1    4   1    4   2    5
    let base = "──────  ──────── ────  ───── ────  ─────── ────────  ─── ──── ────  ─────";
    if show_dist {
        // Dist column: "  Dist" = 6 chars (2 spaces + 4 chars)
        format!("{}  ────", base)
    } else {
        base.to_string()
    }
}

pub fn format_aircraft_table(
    aircraft: &[Aircraft],
    now: i64,
    metric: bool,
    show_dist: bool,
    user_lat: f64,
    user_lon: f64,
) -> Vec<String> {
    let mut rows = vec![
        format_header(metric, show_dist),
        format_separator(show_dist),
    ];
    for a in aircraft {
        let seen_secs = (now - a.seen) / 1000;
        let dist_nm = if show_dist {
            compute_distance_nm(a, user_lat, user_lon)
        } else {
            0.0
        };
        rows.push(format_row(a, metric, show_dist, dist_nm, seen_secs));
    }
    rows
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

pub fn sort_aircraft(aircraft: &mut [Aircraft], sort: SortColumn, user_lat: f64, user_lon: f64) {
    aircraft.sort_by(|a, b| {
        let cmp = match sort {
            SortColumn::Dist => {
                let da = compute_distance_nm(a, user_lat, user_lon);
                let db = compute_distance_nm(b, user_lat, user_lon);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            }
            SortColumn::Alt => {
                let aa = if a.baro_alt != INVALID_ALTITUDE { a.baro_alt } else { 0 };
                let ab = if b.baro_alt != INVALID_ALTITUDE { b.baro_alt } else { 0 };
                aa.cmp(&ab)
            }
            SortColumn::Gs => {
                let ga = if a.gs > 0.0 { a.gs as i32 } else { 0 };
                let gb = if b.gs > 0.0 { b.gs as i32 } else { 0 };
                ga.cmp(&gb)
            }
            SortColumn::Seen => {
                // Most recent first (larger seen = more recent)
                b.seen.cmp(&a.seen)
            }
            SortColumn::Icao => a.addr.cmp(&b.addr),
            SortColumn::Flight => a.callsign.cmp(&b.callsign),
            SortColumn::Registration => a.registration.cmp(&b.registration),
            SortColumn::Type => a.type_code.cmp(&b.type_code),
        };
        // Tie-breaker: ICAO ascending
        cmp.then_with(|| a.addr.cmp(&b.addr))
           .then_with(|| a.seen.cmp(&b.seen))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AddrType};

    fn make_test_aircraft() -> Aircraft {
        let mut a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
        a.callsign = "UAL1234".to_string();
        a.squawk = 0o1234;
        a.baro_alt = 10000;
        a.gs = 250.5;
        a.track = 180.0;
        a.lat = 45.5;
        a.lon = -122.5;
        a.messages = 42;
        a.seen = 1000;
        a.signal_level = [-12.5; 8];
        a.signal_next = 8;
        a
    }

    #[test]
    fn test_format_row_imperial() {
        let a = make_test_aircraft();
        let row = format_row(&a, false, false, 0.0, 4);
        assert!(row.contains("4840D6"), "Row should contain ICAO: {}", row);
        assert!(row.contains("UAL1234"), "Row should contain callsign: {}", row);
        assert!(row.contains("1234"), "Row should contain squawk: {}", row);
        assert!(row.contains("10000"), "Row should contain altitude: {}", row);
        assert!(row.contains("250"), "Row should contain speed: {}", row);
        assert!(row.contains("180"), "Row should contain track: {}", row);
        assert!(row.contains("45.50"), "Row should contain lat: {}", row);
        assert!(row.contains("-122.50"), "Row should contain lon: {}", row);
        assert!(row.contains("42"), "Row should contain message count: {}", row);
        assert!(row.contains("4"), "Row should contain seen: {}", row);
    }

    #[test]
    fn test_format_row_metric() {
        let a = make_test_aircraft();
        let row = format_row(&a, true, false, 0.0, 4);
        // 10000 ft → 3048 m
        assert!(
            row.contains("3048") || row.contains("3047"),
            "Row should contain metric altitude: {}",
            row
        );
        // 250.5 kt → 464 km/h
        assert!(
            row.contains("464") || row.contains("463"),
            "Row should contain metric speed: {}",
            row
        );
    }

    #[test]
    fn test_format_row_with_distance() {
        let a = make_test_aircraft();
        let dist_nm = compute_distance_nm(&a, 45.5, -122.5);
        let row = format_row(&a, false, true, dist_nm, 4);
        // Distance from aircraft to itself should be ~0.0 NM
        assert!(row.contains("0.0"), "Row should contain distance: {}", row);
    }

    #[test]
    fn test_sort_by_dist() {
        let mut aircraft = vec![
            Aircraft { addr: 0xA, lat: 45.0, lon: -122.0, seen: 1000, ..Aircraft::new(0xA, AddrType::AdsbIcao, 1000) },
            Aircraft { addr: 0xB, lat: 45.1, lon: -122.0, seen: 2000, ..Aircraft::new(0xB, AddrType::AdsbIcao, 2000) },
            Aircraft { addr: 0xC, lat: 44.0, lon: -122.0, seen: 3000, ..Aircraft::new(0xC, AddrType::AdsbIcao, 3000) },
        ];
        sort_aircraft(&mut aircraft, SortColumn::Dist, 45.5, -122.5);
        // B (45.1) closest at 0.4°, A (45.0) next at 0.5°, C (44.0) farthest at 1.5°
        assert_eq!(aircraft[0].addr, 0xB); // closest (45.1)
        assert_eq!(aircraft[1].addr, 0xA); // next (45.0)
        assert_eq!(aircraft[2].addr, 0xC); // farthest (44.0)
    }

    #[test]
    fn test_sort_by_seen() {
        let mut aircraft = vec![
            Aircraft { addr: 0xA, seen: 3000, ..Aircraft::new(0xA, AddrType::AdsbIcao, 3000) },
            Aircraft { addr: 0xB, seen: 1000, ..Aircraft::new(0xB, AddrType::AdsbIcao, 1000) },
            Aircraft { addr: 0xC, seen: 2000, ..Aircraft::new(0xC, AddrType::AdsbIcao, 2000) },
        ];
        sort_aircraft(&mut aircraft, SortColumn::Seen, 0.0, 0.0);
        // Most recent first (larger seen = more recent)
        assert_eq!(aircraft[0].addr, 0xA); // seen=3000 (most recent)
        assert_eq!(aircraft[1].addr, 0xC); // seen=2000
        assert_eq!(aircraft[2].addr, 0xB); // seen=1000 (oldest)
    }

    #[test]
    fn test_sort_icao_tiebreaker() {
        let mut aircraft = vec![
            Aircraft { addr: 0xB, baro_alt: 10000, ..Aircraft::new(0xB, AddrType::AdsbIcao, 1000) },
            Aircraft { addr: 0xA, baro_alt: 10000, ..Aircraft::new(0xA, AddrType::AdsbIcao, 1000) },
        ];
        sort_aircraft(&mut aircraft, SortColumn::Alt, 0.0, 0.0);
        // Same altitude, sorted by ICAO ascending
        assert_eq!(aircraft[0].addr, 0xA);
        assert_eq!(aircraft[1].addr, 0xB);
    }

    #[test]
    fn test_format_aircraft_list() {
        let now = 5000i64;
        let aircraft = vec![make_test_aircraft()];
        let rows = format_aircraft_table(&aircraft, now, false, false, 0.0, 0.0);
        assert!(rows[0].contains("ICAO"), "Header should contain ICAO: {}", rows[0]);
        assert!(rows[2].contains("4840D6"), "Row should contain ICAO: {}", rows[2]);
        // seen = (5000 - 1000) / 1000 = 4 seconds
        assert!(rows[2].contains("4"), "Row should contain seen time: {}", rows[2]);
    }

    #[test]
    fn test_compute_distance_nm() {
        let a = Aircraft { addr: 0x1, lat: 45.5, lon: -122.5, ..Aircraft::new(0x1, AddrType::AdsbIcao, 0) };
        let dist = compute_distance_nm(&a, 45.5, -122.5);
        assert!((dist - 0.0).abs() < 0.001, "Distance to self should be ~0: {}", dist);
    }
}
