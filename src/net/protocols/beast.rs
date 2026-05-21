use std::time::{SystemTime, UNIX_EPOCH};
use crate::net::DecodedMessage;

#[allow(dead_code)] // wired from client read_loop; pending Phase 2 Beast framing
pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

/// Encode a decoded message in Beast binary format (MLAT timestamped).
/// Format: DLE STX + 6-byte timestamp MSB + message type + payload + DLE ETX
///
/// Message type:
///   0x31 (ASCII '1') — Mode-S short frame (7 bytes, DF0-16,18-23)
///   0x32 (ASCII '2') — Mode-S long frame  (14 bytes, DF17-18,24-31)
pub fn encode_beast_output(msg: &DecodedMessage) -> Vec<u8> {
    let mut out = Vec::with_capacity(msg.data.len() + 12);

    // Header DLE + STX (MLAT sync)
    out.push(0x10);
    out.push(0x02);

    // Timestamp (6 bytes, big-endian, microseconds since epoch)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    out.extend_from_slice(&now.to_be_bytes()[2..]); // last 6 bytes

    // Message type: 0x31 for short (7-byte) frames, 0x32 for long (14-byte) frames
    let msg_type = if msg.data.len() <= 7 { 0x31 } else { 0x32 };
    out.push(msg_type);

    // Payload
    out.extend_from_slice(&msg.data);

    // Trailer DLE + ETX
    out.push(0x10);
    out.push(0x03);

    out
}
