use crate::console::state::FieldChange;
use crate::console::stats_accumulator::AggregatedStats;
use crate::types::ModesMessage;

pub fn format_low(stats: &AggregatedStats) -> String {
    let df_str: Vec<String> = stats.df_distribution.iter()
        .map(|&(df, count)| {
            let pct = if stats.msgs_per_sec > 0.0 {
                (count as f64 / (stats.msgs_per_sec * 60.0)) * 100.0
            } else { 0.0 };
            format!("DF{:.0}={:.0}%", df, pct)
        })
        .collect();

    let uptime = stats.uptime_secs;
    let hours = uptime / 3600;
    let mins = (uptime % 3600) / 60;

    format!(
        "[readsb] msgs/s={:.0} ac={} ac1h={} crc_bad={:.1}% bitfix={:.1}%\n  sig: {:.1} avg / {:.1} max / {:.1} min dBFS  drops={}\n  df: {}\n  cpr_ok={:.0}% uptime={}h{}m",
        stats.msgs_per_sec,
        stats.currently_tracked,
        stats.unique_aircraft_hour,
        stats.crc_fail_pct,
        stats.bitfix_pct,
        stats.sig_avg,
        stats.sig_max,
        stats.sig_min,
        stats.drops,
        df_str.join(" "),
        stats.cpr_ok_pct,
        hours,
        mins,
    )
}

pub fn format_medium(icao: u32, changes: &[(&str, FieldChange)]) -> String {
    let icao_str = format!("{:06X}", icao);
    if changes.is_empty() {
        return format!("[{}]", icao_str);
    }

    let mut parts: Vec<String> = Vec::new();
    for (label, change) in changes {
        match change {
            FieldChange::Unchanged => {}
            FieldChange::New { value } => {
                parts.push(format!("{}:{}▸", label, value));
            }
            FieldChange::Increased { value, delta } => {
                parts.push(format!("{}:{}▲{:.0}", label, value, delta));
            }
            FieldChange::Decreased { value, delta } => {
                parts.push(format!("{}:{}▼{:.0}", label, value, delta));
            }
            FieldChange::Lost => {
                parts.push(format!("{}:---", label));
            }
        }
    }

    format!("[{}] {}", icao_str, parts.join(" "))
}

fn describe_message_type(msg: &ModesMessage) -> &'static str {
    match msg.msgtype {
        0 | 4 | 16 => match msg.metype {
            5..=8 => "pos",
            9..=19 => "vel",
            _ => "surv",
        },
        5 | 21 => "id",
        11 => "all-call",
        17 | 18 => match msg.metype {
            1..=4 => "surf",
            5..=8 => "pos",
            9..=19 => "vel",
            20 => "st",
            21 | 28 | 31 => "status",
            _ => "adsb",
        },
        19 => "mil",
        20 => "comm-b",
        24 => "comm-d",
        31 => "ext-squitter",
        _ => "unknown",
    }
}

fn format_msg_header(msg: &ModesMessage) -> String {
    let icao = format!("{:06X}", msg.addr);
    let desc = describe_message_type(msg);
    format!("[{}] DF{} {}", icao, msg.msgtype, desc)
}

fn format_msg_fields(msg: &ModesMessage) -> Vec<String> {
    let mut fields = Vec::new();
    if msg.callsign_valid {
        let cs = std::str::from_utf8(&msg.callsign).unwrap_or("").trim_matches('\0').trim();
        if !cs.is_empty() {
            fields.push(format!("callsign:{}", cs));
        }
    }
    if msg.category_valid {
        fields.push(format!("cat:A{}", msg.category));
    }
    if msg.baro_alt_valid {
        fields.push(format!("alt:{}", msg.baro_alt));
    }
    if msg.geom_alt_valid {
        fields.push(format!("alt_geom:{}", msg.geom_alt));
    }
    if msg.gs_valid {
        fields.push(format!("gs:{:.1}", msg.gs));
    }
    if msg.track_valid {
        fields.push(format!("trk:{:.1}", msg.track));
    }
    if msg.baro_rate_valid {
        fields.push(format!("rate:{}", msg.baro_rate));
    }
    if msg.geom_rate_valid {
        fields.push(format!("rate_geom:{}", msg.geom_rate));
    }
    if msg.cpr_decoded {
        fields.push(format!("pos:{:.2},{:.2}", msg.decoded_lat, msg.decoded_lon));
    }
    if msg.cpr_valid {
        fields.push(if msg.cpr_odd { "cpr:odd".into() } else { "cpr:even".into() });
    }
    if msg.squawk_valid {
        fields.push(format!("squawk:{:04x}", msg.squawk_hex));
    }
    if msg.nav.mcp_altitude_valid {
        fields.push(format!("sel_alt:{}", msg.nav.mcp_altitude));
    }
    fields
}

pub fn format_high(msg: &ModesMessage) -> Option<String> {
    if msg.msgtype == 11 || (msg.msgtype == 0 && !msg.baro_alt_valid) {
        return None;
    }
    let header = format_msg_header(msg);
    let fields = format_msg_fields(msg);
    let sig = format!("sig:{:.1}dBFS", msg.signal_level);
    let all = if fields.is_empty() {
        format!("{}  {}", header, sig)
    } else {
        format!("{} {}  {}", header, fields.join(" "), sig)
    };
    Some(all)
}

pub fn format_max(msg: &ModesMessage) -> Option<String> {
    let header = format_msg_header(msg);
    let fields = format_msg_fields(msg);
    let sig = format!("sig:{:.1}dBFS", msg.signal_level);
    let all = if fields.is_empty() {
        format!("{}  {}", header, sig)
    } else {
        format!("{} {}  {}", header, fields.join(" "), sig)
    };
    Some(all)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::console::state::FieldChange;
    use crate::console::stats_accumulator::AggregatedStats;
    use crate::types::ModesMessage;

    #[test]
    fn test_format_low_basic() {
        let stats = AggregatedStats {
            msgs_per_sec: 1845.0,
            currently_tracked: 32,
            unique_aircraft_hour: 47,
            crc_fail_pct: 2.1,
            bitfix_pct: 0.3,
            sig_avg: -12.3,
            sig_max: -8.1,
            sig_min: -31.2,
            drops: 0,
            df_distribution: vec![(17, 1440), (11, 220), (0, 92)],
            cpr_ok_pct: 89.0,
            uptime_secs: 15120,
        };
        let s = format_low(&stats);
        assert!(s.contains("msgs/s=1845"));
        assert!(s.contains("ac=32"));
        assert!(s.contains("ac1h=47"));
        assert!(s.contains("crc_bad=2.1%"));
        assert!(s.contains("-12.3 avg"));
        assert!(s.contains("-8.1 max"));
        assert!(s.contains("DF17"));
        assert!(s.contains("cpr_ok=89%"));
        assert!(s.contains("uptime=4h12m"));
    }

    #[test]
    fn test_format_low_zero_stats() {
        let stats = AggregatedStats {
            msgs_per_sec: 0.0,
            currently_tracked: 0,
            unique_aircraft_hour: 0,
            crc_fail_pct: 0.0,
            bitfix_pct: 0.0,
            sig_avg: 0.0,
            sig_max: 0.0,
            sig_min: 0.0,
            drops: 0,
            df_distribution: vec![],
            cpr_ok_pct: 0.0,
            uptime_secs: 0,
        };
        let s = format_low(&stats);
        assert!(s.contains("msgs/s=0"));
        assert!(s.contains("ac=0"));
        assert!(s.contains("drops=0"));
        assert!(s.contains("uptime=0h0m"));
    }

    #[test]
    fn test_format_low_with_drops() {
        let stats = AggregatedStats {
            msgs_per_sec: 100.0, currently_tracked: 5, unique_aircraft_hour: 10,
            crc_fail_pct: 0.0, bitfix_pct: 0.0,
            sig_avg: -20.0, sig_max: -10.0, sig_min: -30.0, drops: 5,
            df_distribution: vec![], cpr_ok_pct: 100.0, uptime_secs: 3600,
        };
        let s = format_low(&stats);
        assert!(s.contains("drops=5"));
    }

    #[test]
    fn test_format_medium_change_indicators() {
        let changes = vec![
            ("alt", FieldChange::Increased { value: "27600".into(), delta: 500.0 }),
            ("gs", FieldChange::Decreased { value: "378.5".into(), delta: 10.0 }),
            ("callsign", FieldChange::New { value: "ASA1390".into() }),
        ];
        let s = format_medium(0xA43EA2, &changes);
        assert!(s.contains("A43EA2"));
        assert!(s.contains("alt:27600▲"));
        assert!(s.contains("gs:378.5▼"));
        assert!(s.contains("callsign:ASA1390▸"));
    }

    #[test]
    fn test_format_medium_unchanged() {
        let s = format_medium(0xA43EA2, &[]);
        assert!(s.contains("A43EA2"));
        assert_eq!(s, "[A43EA2]");
    }

    #[test]
    fn test_format_medium_lost_field() {
        let changes = vec![("alt", FieldChange::Lost)];
        let s = format_medium(0xA43EA2, &changes);
        assert!(s.contains("alt:---"));
    }

    #[test]
    fn test_format_high_airborne_velocity() {
        let mut msg = ModesMessage::default();
        msg.addr = 0xA43EA2;
        msg.msgtype = 17;
        msg.metype = 19;
        msg.mesub = 1;
        msg.baro_alt_valid = true;
        msg.baro_alt = 27675;
        msg.gs_valid = true;
        msg.gs = 378.5;
        msg.cf = 3;
        msg.baro_rate_valid = true;
        msg.baro_rate = -2240;
        msg.signal_level = -12.1;
        let s = format_high(&msg).unwrap();
        assert!(s.contains("A43EA2"));
        assert!(s.contains("DF17"));
        assert!(s.contains("gs:378.5"));
        assert!(s.contains("alt:27675"));
        assert!(s.contains("sig:-12.1dBFS"));
    }

    #[test]
    fn test_format_high_skips_df11() {
        let mut msg = ModesMessage::default();
        msg.msgtype = 11;
        msg.addr = 0xA6C311;
        msg.signal_level = -24.2;
        assert!(format_high(&msg).is_none());
    }

    #[test]
    fn test_format_high_position() {
        let mut msg = ModesMessage::default();
        msg.addr = 0xA324B0;
        msg.msgtype = 17;
        msg.metype = 5;
        msg.baro_alt_valid = true;
        msg.baro_alt = 27000;
        msg.cpr_decoded = true;
        msg.decoded_lat = 45.34;
        msg.decoded_lon = -121.61;
        msg.cpr_odd = true;
        msg.cpr_valid = true;
        msg.signal_level = -18.1;
        let s = format_high(&msg).unwrap();
        assert!(s.contains("A324B0"));
        assert!(s.contains("pos:45.34,-121.61"));
        assert!(s.contains("cpr:odd"));
        assert!(s.contains("alt:27000"));
    }

    #[test]
    fn test_format_high_callsign() {
        let mut msg = ModesMessage::default();
        msg.addr = 0xA6C311;
        msg.msgtype = 17;
        msg.metype = 4;
        msg.callsign_valid = true;
        msg.category_valid = true;
        msg.category = 3;
        let cs: &[u8] = b"ASA1390\0\0\0\0\0\0\0\0\0";
        msg.callsign[..cs.len()].copy_from_slice(cs);
        msg.signal_level = -24.2;
        let s = format_high(&msg).unwrap();
        assert!(s.contains("callsign:ASA1390"));
        assert!(s.contains("cat:A3"));
    }

    #[test]
    fn test_format_max_includes_df11() {
        let mut msg = ModesMessage::default();
        msg.msgtype = 11;
        msg.addr = 0xA6C311;
        msg.signal_level = -24.2;
        let s = format_max(&msg).unwrap();
        assert!(s.contains("A6C311"));
        assert!(s.contains("DF11"));
        assert!(s.contains("all-call"));
        assert!(s.contains("sig:-24.2dBFS"));
    }

    #[test]
    fn test_format_max_empty_df0() {
        let mut msg = ModesMessage::default();
        msg.msgtype = 0;
        msg.addr = 0xA6C311;
        msg.signal_level = -24.2;
        let s = format_max(&msg).unwrap();
        assert!(s.contains("DF0"));
        assert!(s.contains("surv"));
    }
}
