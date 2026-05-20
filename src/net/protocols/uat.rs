pub struct UatMessage { pub data: Vec<u8> }

pub fn parse_frame(data: &[u8]) -> Option<UatMessage> {
    if data.len() < 5 { return None; }
    Some(UatMessage { data: data.to_vec() })
}
