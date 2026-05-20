pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

pub fn find_frame(data: &[u8]) -> Option<usize> {
    data.windows(2).position(|w| w[0] == 0x10 && w[1] == 0x03)
}
