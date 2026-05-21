#[allow(dead_code)] // wired from client read_loop; pending Phase 2 Beast framing
pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

/// Encode raw Mode-S frame bytes in standard Beast binary format (0x1a-escaped).
///
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload>
///   - 0x1a: frame start marker (not escaped)
///   - type: 0x32 (short, ≤7B) / 0x33 (long, 14B)
///   - timestamp: 6 bytes big-endian, zeros (placeholder)
///   - RSSI: signal/256, clamped to 255, 0xff for signal ≤ 0
///   - payload: raw Mode-S bytes with 0x1a byte-stuffing
pub fn encode_beast_output(data: &[u8], _signal_level: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 10);

    out.push(0x1a); // frame start

    let msg_type = if data.len() <= 7 { 0x32 } else { 0x33 };
    out.push(msg_type);

    // Timestamp: 6 zero bytes (placeholder)
    out.extend_from_slice(&[0u8; 6]);

    // RSSI: hardcoded 0xff for now (mapping added in Task 4)
    out.push(0xff);

    // Payload (byte-stuffing added in Task 2)
    out.extend_from_slice(data);

    out
}
