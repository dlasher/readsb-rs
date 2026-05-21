/// Encode raw Mode-S bytes as AVR-compatible hex line.
/// Format: *<hex string>;\n
pub fn encode_hex_output(data: &[u8]) -> Vec<u8> {
    let hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!("*{};\n", hex).into_bytes()
}

pub fn parse_line(line: &str) -> Option<Vec<u8>> {
    let line = line.trim();
    if line.is_empty() { return None; }
    let hex_str = line.strip_prefix(|c| c == '@' || c == '*')?;
    let hex_str = hex_str.strip_suffix(';')?;
    if hex_str.len() % 2 != 0 { return None; }
    (0..hex_str.len()).step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i+2], 16).ok())
        .collect()
}
