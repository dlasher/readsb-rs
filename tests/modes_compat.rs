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

#[test]
fn test_short_frame_needs_56bit_engine() {
    use readsb::crc::modes_checksum;

    // Build a valid DF11 (56-bit) from scratch with correct CRC
    let mut msg = [0u8; 7];
    msg[0] = 0x5D; // DF11 (01011) + CA (101)
    msg[1..4].copy_from_slice(&[0x48, 0x40, 0xD6]); // ICAO
    let crc_val = modes_checksum(&msg, 56);
    assert_ne!(crc_val, 0, "CRC without embedded field must be non-zero");
    // Embed CRC so msg becomes valid
    msg[4] = (crc_val >> 16) as u8;
    msg[5] = (crc_val >> 8) as u8;
    msg[6] = crc_val as u8;
    assert_eq!(modes_checksum(&msg, 56), 0, "After embedding CRC, message must be valid");

    // Now corrupt it: flip bit 5
    let mut corrupted = msg;
    corrupted[0] ^= 1 << 2; // flip bit 5 (0-indexed from LSB, bit 2 in byte 0)

    // 112-bit engine can't fix 56-bit frames
    let engine_112 = CrcFixEngine::new(112);
    let result_wrong = parse_modes_message(&corrupted, 56, &engine_112, 0.0).unwrap();
    assert!(!result_wrong.crc_ok, "112-bit engine must NOT fix 56-bit frame");

    // 56-bit engine should find and fix the single-bit error
    let corrupt_crc = modes_checksum(&corrupted, 56);
    let engine_56 = CrcFixEngine::new(56);
    let diagnosis = engine_56.diagnose(corrupt_crc);
    assert!(diagnosis.is_some(),
        "56-bit engine must have pattern for CRC 0x{corrupt_crc:06X}");
    assert_eq!(diagnosis.unwrap().errors, 1, "Must be single-bit error");

    let result_correct = parse_modes_message(&corrupted, 56, &engine_56, 0.0).unwrap();
    assert!(result_correct.crc_ok, "56-bit engine must fix 56-bit frame");
    assert_eq!(result_correct.message.addr, 0x4840D6,
        "Fixed DF11 should have correct ICAO");
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) { return None; }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok()).collect()
}
