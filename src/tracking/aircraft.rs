use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::types::*;
use super::validity::DataValidity;

#[derive(Debug, Clone)]
pub struct Aircraft {
    pub addr: u32,
    pub addrtype: AddrType,
    pub seen: i64,
    pub seen_pos: i64,
    pub messages: u32,
    pub lat: f64,
    pub lon: f64,
    pub baro_alt: i32,
    pub geom_alt: i32,
    pub geom_delta: i32,
    pub baro_rate: i32,
    pub geom_rate: i32,
    pub gs: f32,
    pub track: f32,
    pub track_rate: f32,
    pub roll: f32,
    pub ias: u32,
    pub tas: u32,
    pub mach: f64,
    pub mag_heading: f32,
    pub true_heading: f32,
    pub nav_heading: f32,
    pub callsign: String,
    pub squawk: u32,
    pub category: u8,
    pub emergency: Emergency,
    pub airground: AirGround,
    pub cpr_odd_lat: u32,
    pub cpr_odd_lon: u32,
    pub cpr_even_lat: u32,
    pub cpr_even_lon: u32,
    pub callsign_valid: DataValidity,
    pub baro_alt_valid: DataValidity,
    pub geom_alt_valid: DataValidity,
    pub gs_valid: DataValidity,
    pub position_valid: DataValidity,
    pub squawk_valid: DataValidity,
    pub signal_level: [f64; 8],
    pub signal_next: u32,
    pub trace_len: i32,
    pub trace_current_len: i32,
    pub registration: String,
    pub type_code: String,
    pub db_flags: u16,
    pub nav_modes: NavModes,
    pub nav_altitude_mcp: u32,
    pub nav_altitude_fms: u32,
    pub nav_qnh: f32,
    pub pos_nic: u32,
    pub pos_rc: u32,
    pub adsb_version: i32,
    pub receiver_count: u32,
    pub receiver_id: u64,
}

impl Aircraft {
    pub fn new(addr: u32, addrtype: AddrType, now: i64) -> Self {
        Aircraft {
            addr,
            addrtype,
            seen: now,
            seen_pos: 0,
            messages: 0,
            lat: 0.0,
            lon: 0.0,
            baro_alt: INVALID_ALTITUDE,
            geom_alt: INVALID_ALTITUDE,
            geom_delta: 0,
            baro_rate: 0,
            geom_rate: 0,
            gs: 0.0,
            track: 0.0,
            track_rate: 0.0,
            roll: 0.0,
            ias: 0,
            tas: 0,
            mach: 0.0,
            mag_heading: 0.0,
            true_heading: 0.0,
            nav_heading: 0.0,
            callsign: String::new(),
            squawk: 0,
            category: 0,
            emergency: Emergency::None,
            airground: AirGround::Invalid,
            cpr_odd_lat: 0,
            cpr_odd_lon: 0,
            cpr_even_lat: 0,
            cpr_even_lon: 0,
            callsign_valid: DataValidity::new(),
            baro_alt_valid: DataValidity::new(),
            geom_alt_valid: DataValidity::new(),
            gs_valid: DataValidity::new(),
            position_valid: DataValidity::new(),
            squawk_valid: DataValidity::new(),
            signal_level: [0.0; 8],
            signal_next: 0,
            trace_len: 0,
            trace_current_len: 0,
            registration: String::new(),
            type_code: String::new(),
            db_flags: 0,
            nav_modes: NavModes::empty(),
            nav_altitude_mcp: 0,
            nav_altitude_fms: 0,
            nav_qnh: 0.0,
            pos_nic: 0,
            pos_rc: 0,
            adsb_version: -1,
            receiver_count: 0,
            receiver_id: 0,
        }
    }

    pub fn add_signal(&mut self, level: f64, _now: i64) {
        self.signal_level[self.signal_next as usize % 8] = level;
        self.signal_next += 1;
    }

    pub fn get_signal_db(&self) -> f32 {
        let count = self.signal_next.min(8) as usize;
        if count == 0 {
            return 0.0;
        }
        10.0 * (self.signal_level[..count].iter().sum::<f64>() / count as f64 + 1.125e-5).log10() as f32
    }
}

pub struct AircraftRegistry {
    aircraft: RwLock<HashMap<u32, Arc<RwLock<Aircraft>>>>,
}

impl AircraftRegistry {
    pub fn new() -> Self {
        AircraftRegistry {
            aircraft: RwLock::new(HashMap::with_capacity(1024)),
        }
    }

    pub fn get(&self, addr: u32) -> Option<Arc<RwLock<Aircraft>>> {
        self.aircraft.read().unwrap().get(&addr).cloned()
    }

    pub fn get_or_create(&self, addr: u32, now: i64, addrtype: AddrType) -> Arc<RwLock<Aircraft>> {
        self.aircraft
            .write()
            .unwrap()
            .entry(addr)
            .or_insert_with(|| Arc::new(RwLock::new(Aircraft::new(addr, addrtype, now))))
            .clone()
    }

    pub fn remove_stale(&self, now: i64, expire: i64) -> usize {
        let mut map = self.aircraft.write().unwrap();
        let before = map.len();
        map.retain(|_, a| now < a.read().unwrap().seen + expire);
        before - map.len()
    }

    pub fn len(&self) -> usize {
        self.aircraft.read().unwrap().len()
    }
}

impl Default for AircraftRegistry {
    fn default() -> Self {
        Self::new()
    }
}
