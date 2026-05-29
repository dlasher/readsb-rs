use crate::types::{AirGround, ModesMessage};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConsoleLevel {
    Low,
    Medium,
    High,
    Max,
}

pub fn cycle_level(current: ConsoleLevel, forward: bool) -> ConsoleLevel {
    use ConsoleLevel::*;
    if forward {
        match current {
            Low => Medium,
            Medium => High,
            High => Max,
            Max => Low,
        }
    } else {
        match current {
            Low => Max,
            Max => High,
            High => Medium,
            Medium => Low,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AircraftSnapshot {
    pub icao: u32,
    pub baro_alt: Option<i32>,
    pub geom_alt: Option<i32>,
    pub gs: Option<f32>,
    pub track: Option<f32>,
    pub baro_rate: Option<i32>,
    pub callsign: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub signal_level: Option<f64>,
    pub squawk: Option<u32>,
    pub category: Option<u8>,
    pub airground: Option<AirGround>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldChange {
    Unchanged,
    New { value: String },
    Increased { value: String, delta: f64 },
    Decreased { value: String, delta: f64 },
    Lost,
}

fn field_change(label: &str, prev: Option<f64>, current: Option<f64>) -> (&str, FieldChange) {
    match (prev, current) {
        (None, None) => (label, FieldChange::Unchanged),
        (None, Some(v)) => (label, FieldChange::New { value: format_field(v, label) }),
        (Some(_), None) => (label, FieldChange::Lost),
        (Some(p), Some(c)) => {
            if (c - p).abs() < f64::EPSILON {
                (label, FieldChange::Unchanged)
            } else if c > p {
                (label, FieldChange::Increased { value: format_field(c, label), delta: c - p })
            } else {
                (label, FieldChange::Decreased { value: format_field(c, label), delta: p - c })
            }
        }
    }
}

fn format_field(v: f64, label: &str) -> String {
    if label == "alt" || label == "rate" {
        format!("{:.0}", v)
    } else if label == "sig" || label == "gs" || label == "trk" {
        format!("{:.1}", v)
    } else {
        format!("{}", v)
    }
}

pub fn diff_snapshots<'a>(prev: &'a AircraftSnapshot, current: &'a AircraftSnapshot) -> Vec<(&'a str, FieldChange)> {
    let mut changes: Vec<(&'a str, FieldChange)> = Vec::new();
    for (label, p, c) in &[
        ("alt", prev.baro_alt.map(|v| v as f64), current.baro_alt.map(|v| v as f64)),
        ("alt_geom", prev.geom_alt.map(|v| v as f64), current.geom_alt.map(|v| v as f64)),
        ("gs", prev.gs.map(|v| v as f64), current.gs.map(|v| v as f64)),
        ("trk", prev.track.map(|v| v as f64), current.track.map(|v| v as f64)),
        ("rate", prev.baro_rate.map(|v| v as f64), current.baro_rate.map(|v| v as f64)),
        ("sig", prev.signal_level, current.signal_level),
    ] {
        let (l, fc) = field_change(label, *p, *c);
        if fc != FieldChange::Unchanged {
            changes.push((l, fc));
        }
    }

    match (&prev.callsign, &current.callsign) {
        (None, None) => {}
        (None, Some(cs)) => changes.push(("callsign", FieldChange::New { value: cs.clone() })),
        (Some(_), None) => changes.push(("callsign", FieldChange::Lost)),
        (Some(p), Some(c)) if p != c => changes.push(("callsign", FieldChange::New { value: c.clone() })),
        _ => {}
    }

    match (prev.lat.zip(prev.lon), current.lat.zip(current.lon)) {
        (None, None) => {}
        (None, Some((lat, lon))) => {
            changes.push(("pos", FieldChange::New { value: format!("{:.2},{:.2}", lat, lon) }));
        }
        (Some(_), None) => changes.push(("pos", FieldChange::Lost)),
        (Some((pl, pn)), Some((cl, cn))) => {
            if (cl - pl).abs() > 0.001 || (cn - pn).abs() > 0.001 {
                changes.push(("pos", FieldChange::New { value: format!("{:.2},{:.2}", cl, cn) }));
            }
        }
    }

    changes
}

pub fn snapshot_from_message(msg: &ModesMessage) -> AircraftSnapshot {
    let callsign_str = if msg.callsign_valid && msg.callsign.iter().any(|&b| b != 0) {
        Some(
            std::str::from_utf8(&msg.callsign)
                .unwrap_or("")
                .trim_matches('\0')
                .trim()
                .to_string(),
        )
    } else {
        None
    };

    AircraftSnapshot {
        icao: msg.addr,
        baro_alt: if msg.baro_alt_valid { Some(msg.baro_alt) } else { None },
        geom_alt: if msg.geom_alt_valid { Some(msg.geom_alt) } else { None },
        gs: if msg.gs_valid { Some(msg.gs) } else { None },
        track: if msg.track_valid { Some(msg.track) } else { None },
        baro_rate: if msg.baro_rate_valid { Some(msg.baro_rate) } else { None },
        callsign: callsign_str,
        lat: if msg.cpr_decoded { Some(msg.decoded_lat) } else { None },
        lon: if msg.cpr_decoded { Some(msg.decoded_lon) } else { None },
        signal_level: if msg.signal_level != 0.0 { Some(msg.signal_level) } else { None },
        squawk: if msg.squawk_valid { Some(msg.squawk_hex) } else { None },
        category: if msg.category_valid { Some(msg.category) } else { None },
        airground: Some(msg.airground),
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::types::ModesMessage;

    #[test]
    fn test_cycle_level_forward() {
        use ConsoleLevel::*;
        assert_eq!(cycle_level(Low, true), Medium);
        assert_eq!(cycle_level(Medium, true), High);
        assert_eq!(cycle_level(High, true), Max);
        assert_eq!(cycle_level(Max, true), Low);
    }

    #[test]
    fn test_cycle_level_backward() {
        use ConsoleLevel::*;
        assert_eq!(cycle_level(Low, false), Max);
        assert_eq!(cycle_level(Max, false), High);
        assert_eq!(cycle_level(High, false), Medium);
        assert_eq!(cycle_level(Medium, false), Low);
    }

    #[test]
    fn test_diff_snapshots_all_unchanged() {
        let s = AircraftSnapshot {
            icao: 0xA43EA2, baro_alt: Some(27600), geom_alt: None,
            gs: Some(378.5), track: Some(352.3), baro_rate: Some(-2240),
            callsign: Some("ASA1390".into()), lat: None, lon: None,
            signal_level: Some(-12.1), squawk: None, category: None,
            airground: Some(AirGround::Airborne),
        };
        let changes = diff_snapshots(&s, &s);
        assert!(changes.is_empty(), "identical snapshots should produce no changes: {:?}", changes);
    }

    #[test]
    fn test_diff_snapshots_alt_changed() {
        let prev = AircraftSnapshot {
            icao: 0xA43EA2, baro_alt: Some(10000), geom_alt: None,
            gs: None, track: None, baro_rate: None,
            callsign: None, lat: None, lon: None,
            signal_level: None, squawk: None, category: None,
            airground: Some(AirGround::Invalid),
        };
        let curr = AircraftSnapshot {
            baro_alt: Some(10500), ..prev.clone()
        };
        let changes = diff_snapshots(&prev, &curr);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "alt");
        assert_eq!(changes[0].1, FieldChange::Increased { value: "10500".into(), delta: 500.0 });
    }

    #[test]
    fn test_diff_snapshots_field_lost() {
        let prev = AircraftSnapshot {
            icao: 0xA43EA2, baro_alt: Some(10000), geom_alt: None,
            gs: None, track: None, baro_rate: None,
            callsign: None, lat: None, lon: None,
            signal_level: None, squawk: None, category: None,
            airground: Some(AirGround::Invalid),
        };
        let curr = AircraftSnapshot {
            baro_alt: None, ..prev.clone()
        };
        let changes = diff_snapshots(&prev, &curr);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, FieldChange::Lost);
    }

    #[test]
    fn test_snapshot_from_message() {
        let mut msg = ModesMessage::default();
        msg.addr = 0xA43EA2;
        msg.baro_alt_valid = true;
        msg.baro_alt = 27600;
        msg.gs_valid = true;
        msg.gs = 378.5;
        let snapshot = snapshot_from_message(&msg);
        assert_eq!(snapshot.icao, 0xA43EA2);
        assert_eq!(snapshot.baro_alt, Some(27600));
        assert_eq!(snapshot.gs, Some(378.5));
        assert!(snapshot.callsign.is_none());
    }

    #[test]
    fn test_snapshot_from_message_with_callsign() {
        let mut msg = ModesMessage::default();
        msg.addr = 0xA6C311;
        msg.callsign_valid = true;
        let cs: &[u8] = b"ASA1390\0\0\0\0\0\0\0\0\0";
        msg.callsign[..cs.len()].copy_from_slice(cs);
        let snapshot = snapshot_from_message(&msg);
        assert_eq!(snapshot.callsign.as_deref(), Some("ASA1390"));
    }

    #[test]
    fn test_diff_snapshots_callsign_new() {
        let prev = AircraftSnapshot {
            icao: 0xA6C311, baro_alt: None, geom_alt: None,
            gs: None, track: None, baro_rate: None,
            callsign: None, lat: None, lon: None,
            signal_level: None, squawk: None, category: None,
            airground: Some(AirGround::Invalid),
        };
        let curr = AircraftSnapshot {
            callsign: Some("ASA1390".into()), ..prev.clone()
        };
        let changes = diff_snapshots(&prev, &curr);
        assert!(changes.iter().any(|(l, _)| *l == "callsign"));
    }
}
