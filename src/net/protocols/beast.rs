use std::time::{SystemTime, UNIX_EPOCH};
use crate::net::DecodedMessage;

pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

pub fn find_frame(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w[0] == 0x10 && w[1] == 0x03)
}

/// Encode a decoded message in Beast binary format.
/// Format: DLE (0x10) ETX (0x03) + 6-byte timestamp MSB + message type + payload + DLE + ETX
pub fn encode_beast_output(msg: &DecodedMessage) -> Vec<u8> {
    let mut out = Vec::with_capacity(msg.data.len() + 12);

    // Header DLE + ETX
    out.push(0x10);
    out.push(0x03);

    // Timestamp (6 bytes, big-endian, microseconds since epoch)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64;
    out.extend_from_slice(&now.to_be_bytes()[2..]); // last 6 bytes

    // Message type — derive from payload first byte (DF type)
    let msg_type = msg.data.first().map(|b| (b >> 3) & 0x1F).unwrap_or(0);
    out.push(msg_type);

    // Payload
    out.extend_from_slice(&msg.data);

    // Trailer DLE + ETX
    out.push(0x10);
    out.push(0x03);

    out
}
