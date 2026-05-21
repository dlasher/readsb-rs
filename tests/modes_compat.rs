use readsb::crc::CrcFixEngine;
use readsb::modes::parse_modes_message;

#[test]
fn test_parse_df17_position() {
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&bytes, 112, &engine, 0.0).unwrap();
    assert!(result.crc_ok);
    assert_eq!(result.message.msgtype, 17);
    assert_eq!(result.message.addr, 0x4840D6);
}

#[test]
fn test_parse_modes_message_with_signal() {
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let signal = 5000.0;
    let result = parse_modes_message(&bytes, 112, &engine, signal).unwrap();
    assert_eq!(result.message.signal_level, signal);
}

#[test]
fn test_parse_df11_all_call() {
    let bytes = hex_to_bytes("5D4840D6202CCC").unwrap();
    let engine = CrcFixEngine::new(56);
    let result = parse_modes_message(&bytes, 56, &engine, 0.0).unwrap();
    assert_eq!(result.message.msgtype, 11);
    assert_eq!(result.message.addr, 0x4840D6);
}

#[test]
fn test_parse_invalid_length() {
    let engine = CrcFixEngine::new(112);
    assert!(parse_modes_message(&[0u8; 10], 80, &engine, 0.0).is_none());
}

#[test]
fn test_cpr_decoded_set_on_airborne_position() {
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&bytes, 112, &engine, 0.0).unwrap();
    assert!(result.message.cpr_valid, "Message should have valid CPR");
    assert!(result.message.cpr_decoded, "cpr_decoded should be true for airborne position");
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 { return None; }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok()).collect()
}
