use std::sync::Arc;
use crate::types::*;
use super::{AircraftRegistry};

pub struct Tracker {
    pub registry: AircraftRegistry,
    pub json_reliable: i32,
    pub max_range: f64,
    pub user_lat: f64,
    pub user_lon: f64,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker {
            registry: AircraftRegistry::new(),
            json_reliable: 2,
            max_range: 300.0 * 1852.0,
            user_lat: 0.0,
            user_lon: 0.0,
        }
    }

    pub fn update_from_message(&self, msg: &ModesMessage, now: i64) {
        let addr = msg.addr;
        let aircraft = self.registry.get_or_create(addr, now, AddrType::Unknown);

        let mut a = aircraft.write().unwrap();

        if msg.baro_alt_valid {
            a.baro_alt = msg.baro_alt;
        }

        if msg.gs_valid {
            a.gs = msg.gs;
        }

        if msg.track_valid {
            a.track = msg.gs;
        }

        if msg.baro_rate_valid {
            a.baro_rate = msg.baro_rate;
        }

        if msg.cpr_valid {
            a.lat = msg.decoded_lat;
            a.lon = msg.decoded_lon;
        }

        a.messages += 1;
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}
