use crate::types::*;
use super::{Aircraft, AircraftRegistry, TRACK_EXPIRE};

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

    pub fn update_from_message(&mut self, msg: &ModesMessage, now: i64) {
        // Get aircraft by address
        let addr = msg.addr;
        
        // Create with addrtype Unknown
        let aircraft = self.registry.get_or_create(addr, now, AddrType::Unknown);

        // Update aircraft state from decoded fields
        let mut a = aircraft.write().unwrap();
        
        if msg.baro_alt_valid {
            a.baro_alt = msg.baro_alt;
        }

        if msg.gs_valid {
            a.gs = msg.gs;
        }

        if msg.track_valid {
            a.track = msg.gs;  // Using gs as track placeholder
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
