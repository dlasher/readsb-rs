/// AIS 6-bit character encoding table (ais_charset.c).
pub const AIS_CHARSET: [char; 64] = [
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G',
    'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W',
    'X', 'Y', 'Z', '[', '\\', ']', '^', '_',
    ' ', '!', '"', '#', '$', '%', '&', '\'',
    '(', ')', '*', '+', ',', '-', '.', '/',
    '0', '1', '2', '3', '4', '5', '6', '7',
    '8', '9', ':', ';', '<', '=', '>', '?',
];

/// Convert an AIS 6-bit character to ASCII.
pub fn ais_6bit_to_ascii(ch: u8) -> char {
    if (ch as usize) < AIS_CHARSET.len() {
        AIS_CHARSET[ch as usize]
    } else {
        ' '
    }
}

/// Pack 6-bit values into bytes.
pub fn pack_6bit_to_bytes(values: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut bit_buffer: u32 = 0;
    let mut bit_count = 0;
    for &val in values {
        bit_buffer = (bit_buffer << 6) | (val as u32);
        bit_count += 6;
        while bit_count >= 8 {
            bit_count -= 8;
            bytes.push((bit_buffer >> bit_count) as u8);
        }
    }
    bytes
}
