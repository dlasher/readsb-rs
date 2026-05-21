use std::sync::{Arc, RwLock};
use crate::types::*;
use crate::cpr::decode_cpr_airborne;
use super::{Aircraft, AircraftRegistry, TRACK_EXPIRE};

pub struct Tracker {
    pub registry: AircraftRegistry,
    pub json_reliable: i32,
    pub max_range: f64,
    pub user_lat: f64,
    pub user_lon: f64,
}

impl Tracker {
    fn update_altitude(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
        if msg.baro_alt_valid {
            a.baro_alt_valid.update(msg.source, now);
            a.baro_alt = msg.baro_alt;
        }
        if msg.geom_alt_valid {
            a.geom_alt_valid.update(msg.source, now);
            a.geom_alt = msg.geom_alt;
        }
        if a.baro_alt_valid.is_valid(now, TRACK_EXPIRE) && a.geom_alt_valid.is_valid(now, TRACK_EXPIRE) {
            a.geom_delta = a.geom_alt - a.baro_alt;
        }
    }

    pub fn new() -> Self {
        Tracker {
            registry: AircraftRegistry::new(),
            json_reliable: 2,
            max_range: 300.0 * 1852.0,
            user_lat: 0.0,
            user_lon: 0.0,
        }
    }

    fn update_callsign(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
        if msg.callsign_valid {
            let cs = String::from_utf8_lossy(&msg.callsign).trim_end_matches('\0').to_string();
            if !cs.is_empty() {
                a.callsign_valid.update(msg.source, now);
                a.callsign = cs;
            }
        }
    }

    fn update_velocity(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
        if msg.gs_valid {
            a.gs_valid.update(msg.source, now);
            a.gs = msg.gs;
        }
        if msg.ias_valid {
            a.ias = msg.ias;
        }
        if msg.tas_valid {
            a.tas = msg.tas;
        }
        if msg.mach_valid {
            a.mach = msg.mach;
        }
    }

    fn update_squawk(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
        if msg.squawk_valid {
            a.squawk_valid.update(msg.source, now);
            a.squawk = msg.squawk_hex;
        }
    }

    fn update_emergency(&self, a: &mut Aircraft, msg: &ModesMessage) {
        if msg.emergency != Emergency::None {
            a.emergency = msg.emergency;
        }
    }

    fn update_nav(&self, a: &mut Aircraft, msg: &ModesMessage) {
        if msg.nav.mcp_altitude_valid {
            a.nav_altitude_mcp = msg.nav.mcp_altitude;
        }
        if msg.nav.fms_altitude_valid {
            a.nav_altitude_fms = msg.nav.fms_altitude;
        }
        if msg.nav.qnh_valid {
            a.nav_qnh = msg.nav.qnh;
        }
    }

    fn update_accuracy(&self, a: &mut Aircraft, msg: &ModesMessage) {
        a.pos_nic = msg.accuracy.nac_p;
        a.pos_rc = msg.accuracy.nac_p;
    }

    fn update_position(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
        if !msg.cpr_valid {
            return;
        }
        if msg.cpr_odd {
            a.cpr_odd_lat = msg.cpr_lat;
            a.cpr_odd_lon = msg.cpr_lon;
        } else {
            a.cpr_even_lat = msg.cpr_lat;
            a.cpr_even_lon = msg.cpr_lon;
        }
        // Decode when we have both even and odd frames
        if a.cpr_even_lat != 0 || a.cpr_even_lon != 0 {
            if a.cpr_odd_lat != 0 || a.cpr_odd_lon != 0 {
                if let Some((lat, lon)) = decode_cpr_airborne(
                    a.cpr_even_lat as i32, a.cpr_even_lon as i32,
                    a.cpr_odd_lat as i32, a.cpr_odd_lon as i32,
                    0,
                ) {
                    if lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                        a.lat = lat;
                        a.lon = lon;
                        a.position_valid.update(msg.source, now);
                        a.seen_pos = now;
                    }
                }
            }
        }
    }

    pub fn remove_stale(&self, now: i64) -> usize {
        self.registry.remove_stale(now, TRACK_EXPIRE)
    }

    pub fn update_from_message(&self, msg: &ModesMessage, now: i64) -> Option<Arc<RwLock<Aircraft>>> {
        if msg.addr == 0 || msg.addr == 0xFFFFFF {
            return None;
        }
        let aircraft = self.registry.get_or_create(msg.addr, now, msg.addrtype);

        {
            let mut a = aircraft.write().unwrap();
            self.update_altitude(&mut a, msg, now);
            self.update_callsign(&mut a, msg, now);
            self.update_velocity(&mut a, msg, now);
            self.update_squawk(&mut a, msg, now);
            self.update_emergency(&mut a, msg);
            self.update_nav(&mut a, msg);
            self.update_accuracy(&mut a, msg);
            self.update_position(&mut a, msg, now);
            if msg.track_valid {
                a.track = msg.gs;
            }
            if msg.baro_rate_valid {
                a.baro_rate = msg.baro_rate;
            }
            a.messages += 1;
        }

        Some(aircraft)
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}
