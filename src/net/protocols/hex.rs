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
