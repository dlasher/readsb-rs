pub use crate::types::comm_b::CommBFormat;

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
