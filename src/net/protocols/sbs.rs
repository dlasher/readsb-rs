use crate::tracking::Aircraft;

/// Convert epoch milliseconds to (YYYY/MM/DD, HH:mm:ss.SSS) timestamp strings.
fn sbs_timestamp(now_ms: i64) -> (String, String) {
    let secs = now_ms / 1000;
    let millis = now_ms % 1000;

    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let mut y = 1970i64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    let mut d = remaining_days;
    while d >= month_days[m] {
        d -= month_days[m];
        m += 1;
    }

    let date = format!("{:04}/{:02}/{:02}", y, m + 1, d + 1);
    let time = format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis);
    (date, time)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Encode an aircraft's current state as SBS Basestation CSV lines.
/// Always emits MSG,7 (ICAO) and MSG,8 (signal).
pub fn encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8> {
    let icao = format!("{:06X}", a.addr);
    let (date, time) = sbs_timestamp(now_ms);

    let mut lines = Vec::new();

    lines.push(format!(
        "MSG,7,1,1,{},1,{},{},{},{},,,,,,,,,,,,,\r\n",
        icao, date, time, date, time
    ));

    let signal_db = a.get_signal_db();
    lines.push(format!(
        "MSG,8,1,1,{},1,{},{},{},{},,,,,,,,,,,{:.1},,,,\r\n",
        icao, date, time, date, time, signal_db
    ));

    lines.join("").into_bytes()
}

#[allow(dead_code)] // wired from client read_loop; pending Phase 2 inbound parsing
pub struct SbsMessage {
    pub hex_ident: String,
    pub altitude: i32,
    pub ground_speed: f64,
    pub track: f64,
    pub lat: f64,
    pub lon: f64,
    pub vertical_rate: i32,
    pub squawk: String,
    pub callsign: String,
}

pub fn parse_line(line: &str) -> Option<SbsMessage> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 22 { return None; }
    if fields[0] != "MSG" { return None; }

    Some(SbsMessage {
        hex_ident: fields.get(4).unwrap_or(&"").to_string(),
        altitude: fields.get(11).and_then(|s| s.parse().ok()).unwrap_or(0),
        ground_speed: fields.get(12).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        track: fields.get(13).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        lat: fields.get(14).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        lon: fields.get(15).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        vertical_rate: fields.get(16).and_then(|s| s.parse().ok()).unwrap_or(0),
        squawk: fields.get(17).unwrap_or(&"").to_string(),
        callsign: fields.get(10).unwrap_or(&"").to_string(),
    })
}
