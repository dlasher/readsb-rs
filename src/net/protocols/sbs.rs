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
