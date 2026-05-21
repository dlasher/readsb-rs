use readsb::net::protocols::{beast, sbs, hex};

#[test]
fn test_beast_parse_timestamp() {
    let ts_bytes = [0x00, 0x00, 0x01, 0x02, 0x03, 0x04];
    let ts = beast::parse_timestamp(&ts_bytes);
    assert_eq!(ts, Some(0x01020304));
}

#[test]
fn test_sbs_parse_basic() {
    let line = "MSG,3,1,1,4840D6,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,BAW123,35000,220,45,51.5,-0.5,0,0,0,0,0,0";
    let msg = sbs::parse_line(line);
    assert!(msg.is_some());
    let m = msg.unwrap();
    assert_eq!(m.hex_ident, "4840D6");
    assert_eq!(m.callsign, "BAW123");
}

#[test]
fn test_hex_parse_basic() {
    let result = hex::parse_line("*8D4840D6202CC371C32CE0576098;");
    assert!(result.is_some());
}

#[test]
fn test_hex_parse_empty() {
    assert!(hex::parse_line("").is_none());
}

#[test]
fn test_beast_encode_output() {
    let msg = readsb::net::DecodedMessage {
        data: vec![0x1A, 0x2B, 0x3C, 0x4D],
        client_id: 0,
    };
    let encoded = beast::encode_beast_output(&msg);
    // Beast MLAT format: DLE STX + 6-byte timestamp + type + payload + DLE ETX
    assert!(!encoded.is_empty(), "Encoded output should not be empty");
    assert_eq!(encoded[0], 0x10, "Should start with DLE");
    assert_eq!(encoded[1], 0x02, "Should start with DLE STX (MLAT sync)");
    // Last two bytes should be DLE ETX terminator
    assert_eq!(encoded[encoded.len() - 2], 0x10, "Should end with DLE");
    assert_eq!(encoded[encoded.len() - 1], 0x03, "Should end with ETX");
    // Type byte (position 8) should be 0x31 (short, payload ≤ 7 bytes)
    assert_eq!(encoded[8], 0x31, "Type byte should be 0x31 (short frame) for 4-byte payload");
    // There should be at least 8 bytes: DLE STX + 6-byte timestamp + type + payload + DLE ETX
    assert!(encoded.len() >= 8 + msg.data.len(),
        "Encoded length should accommodate header + payload + trailer");
}
