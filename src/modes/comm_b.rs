pub use crate::types::comm_b::CommBFormat;
use crate::modes::ais::ais_6bit_to_ascii;

/// Decode a Comm-B message from MB bytes.
/// Returns the inferred Comm-B format based on BDS register.
pub fn decode_comm_b(mb: &[u8; 7]) -> CommBFormat {
    let bds = mb[0]; // BDS register address
    match bds {
        0x10 | 0x20 => CommBFormat::AircraftIdent,
        0x30 => {
            if mb[1..].iter().all(|&b| b == 0) {
                CommBFormat::EmptyResponse
            } else {
                CommBFormat::AcasRA
            }
        }
        0x40 => CommBFormat::VerticalIntent,
        0x50 => CommBFormat::TrackTurn,
        0x60 => CommBFormat::HeadingSpeed,
        0x17 => CommBFormat::DatalinkCaps,
        0x18 => CommBFormat::GicbCaps,
        0x80 => CommBFormat::MeteorologicalRoutine,
        _ => {
            if mb[1..].iter().all(|&b| b == 0) {
                CommBFormat::EmptyResponse
            } else {
                CommBFormat::Unknown
            }
        }
    }
}

/// Extract ACARS-style callsign from Comm-B aircraft ident MB field.
pub fn commb_callsign(mb: &[u8]) -> String {
    let mut chars = String::with_capacity(8);
    let bits: u64 = mb.iter().take(7).fold(0u64, |acc, &b| (acc << 8) | b as u64);
    for i in (0..42).step_by(6).rev() {
        let ch = ((bits >> i) & 0x3F) as u8;
        let c = ais_6bit_to_ascii(ch);
        if c == '@' { break; }
        chars.push(c);
    }
    chars.trim().to_string()
}
