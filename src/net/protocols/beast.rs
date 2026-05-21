/// A decoded Beast protocol frame (input side).
pub struct BeastFrame {
    pub timestamp: i64,
    pub frame_type: u8,
    pub payload: Vec<u8>,
    pub rssi: u8,
}

/// Parse a single Beast frame from a buffer starting at offset 0.
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload (byte-stuffed)>
pub fn parse_beast_frame(data: &[u8]) -> Option<BeastFrame> {
    if data.len() < 9 { return None; }
    if data[0] != 0x1a { return None; }
    if data[1] != 0x32 && data[1] != 0x33 { return None; }

    let ts_bytes = &data[2..8];
    let timestamp = i64::from_be_bytes([0, 0, ts_bytes[0], ts_bytes[1], ts_bytes[2], ts_bytes[3], ts_bytes[4], ts_bytes[5]]);
    let rssi = data[8];
    let raw_payload = &data[9..];
    if raw_payload.is_empty() { return None; }

    let payload = destuff_beast(raw_payload);
    Some(BeastFrame { timestamp, frame_type: data[1], payload, rssi })
}

/// Inverse of escape_beast: collapse 0x1a 0x1a → 0x1a.
pub fn destuff_beast(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        out.push(data[i]);
        if data[i] == 0x1a && i + 1 < data.len() && data[i + 1] == 0x1a {
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

#[allow(dead_code)] // wired from client read_loop; pending Phase 2 Beast framing
pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

/// Escape 0x1a bytes by doubling them (standard Beast byte-stuffing).
fn escape_beast(data: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(data.len());
    for &b in data {
        escaped.push(b);
        if b == 0x1a {
            escaped.push(0x1a);
        }
    }
    escaped
}

/// Encode raw Mode-S frame bytes in standard Beast binary format (0x1a-escaped).
///
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload>
///   - 0x1a: frame start marker (not escaped)
///   - type: 0x32 (short, ≤7B) / 0x33 (long, 14B)
///   - timestamp: 6 bytes big-endian, zeros (placeholder)
///   - RSSI: signal/256, clamped to 255, 0xff for signal ≤ 0
///   - payload: raw Mode-S bytes with 0x1a byte-stuffing
pub fn encode_beast_output(data: &[u8], signal_level: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 10);

    out.push(0x1a); // frame start

    let msg_type = if data.len() <= 7 { 0x32 } else { 0x33 };
    out.push(msg_type);

    // Timestamp: 6 zero bytes (placeholder)
    out.extend_from_slice(&[0u8; 6]);

    // RSSI: signal_level/256, 0xff sentinel if no signal
    let rssi = if signal_level <= 0.0 {
        0xff
    } else {
        ((signal_level / 256.0).min(255.0)) as u8
    };
    out.push(rssi);

    // Payload with byte-stuffing
    out.extend_from_slice(&escape_beast(data));

    out
}
