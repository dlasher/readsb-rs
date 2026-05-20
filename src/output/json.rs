use serde::Serialize;
use crate::types::*;
use crate::tracking::Aircraft;

#[derive(Serialize)]
pub struct AircraftJson {
    pub hex: String,
    pub flight: Option<String>,
    pub alt_baro: Option<i32>,
    pub alt_geom: Option<i32>,
    pub gs: Option<f32>,
    pub track: Option<f32>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub squawk: Option<String>,
    pub emergency: Option<String>,
    pub category: Option<u8>,
    pub messages: u32,
    pub seen: i64,
}

impl AircraftJson {
    pub fn from_aircraft(a: &Aircraft, now: i64) -> Self {
        AircraftJson {
            hex: format!("{:06X}", a.addr),
            flight: if a.callsign.is_empty() { None } else { Some(a.callsign.clone()) },
            alt_baro: if a.baro_alt != INVALID_ALTITUDE { Some(a.baro_alt) } else { None },
            alt_geom: if a.geom_alt != INVALID_ALTITUDE { Some(a.geom_alt) } else { None },
            gs: if a.gs > 0.0 { Some(a.gs) } else { None },
            track: if a.track >= 0.0 { Some(a.track) } else { None },
            lat: if a.lat != 0.0 { Some(a.lat) } else { None },
            lon: if a.lon != 0.0 { Some(a.lon) } else { None },
            squawk: if a.squawk > 0 { Some(format!("{:04o}", a.squawk)) } else { None },
            emergency: match a.emergency { Emergency::None => None, _ => Some(format!("{:?}", a.emergency)) },
            category: if a.category > 0 { Some(a.category) } else { None },
            messages: a.messages,
            seen: now - a.seen,
        }
    }
}

pub fn generate_aircraft_json(aircraft: Vec<Aircraft>, now: i64) -> String {
    let json_list: Vec<AircraftJson> = aircraft.iter().map(|a| AircraftJson::from_aircraft(a, now)).collect();
    serde_json::to_string(&json_list).unwrap_or_else(|_| "[]".to_string())
}
