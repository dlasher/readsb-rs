/// Detect a Mode A/C reply from magnitude samples.
/// Returns the 13-bit Mode A code if a valid preamble is detected.
pub fn detect_mode_a(samples: &[u16]) -> Option<u32> {
    // Mode A/C preamble: pulses at sample positions 0 and 3
    if samples.len() < 20 || samples[0] < 32768 || samples[3] < 32768 {
        return None;
    }
    let mut code = 0u32;
    for i in 0..13 {
        if samples.get(4 + i).copied().unwrap_or(0) > 32768 {
            code |= 1 << i;
        }
    }
    Some(code)
}

/// Convert Mode A code to Mode C altitude in feet.
/// Returns INVALID_ALTITUDE (-9999) if the code cannot be decoded.
pub fn mode_a_to_mode_c(mode_a: u32) -> i32 {
    // Gillham code decoding
    let c1 = (mode_a & 0x0001) != 0;
    let a1 = (mode_a & 0x0010) != 0;
    let c2 = (mode_a & 0x0100) != 0;
    let a2 = (mode_a & 0x1000) != 0;
    let c4 = (mode_a & 0x0002) != 0;
    let a4 = (mode_a & 0x0020) != 0;

    // Invalid if two C bits or two A bits are set in the same group
    if (c1 && c2 && c4) || (a1 && a2 && a4) {
        return -9999;
    }

    let five_hundreds = if a4 { 4 } else { 0 } + if a2 { 2 } else { 0 } + if a1 { 1 } else { 0 };
    let one_hundreds = if c4 { 4 } else { 0 } + if c2 { 2 } else { 0 } + if c1 { 1 } else { 0 };

    ((five_hundreds * 5 + one_hundreds) as i32 - 10) * 100
}
