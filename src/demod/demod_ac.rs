use crate::modes::mode_ac::{detect_mode_a, mode_a_to_mode_c};

/// Demodulate Mode A/C replies from magnitude samples.
/// Returns None if no valid preamble found, otherwise Some((mode_a_code, spi_bit)).
pub fn demodulate_ac(mag: &[u16]) -> Option<(u32, bool)> {
    detect_mode_a(mag).map(|code| (code, false))
}

/// Convert Mode A code to Mode C altitude.
pub fn modeac_to_altitude(code: u32) -> i32 {
    mode_a_to_mode_c(code)
}
