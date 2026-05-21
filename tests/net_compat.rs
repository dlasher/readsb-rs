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
    let _encoded = beast::encode_beast_output(&msg.data, 0.0);
}

#[test]
fn test_beast_frame_types() {
    use readsb::net::protocols::beast;

    let encoded_short = beast::encode_beast_output(&[0u8; 7], 0.0);
    assert_eq!(encoded_short[1], 0x32, "7B payload → type 0x32");

    let encoded_long = beast::encode_beast_output(&[0u8; 14], 0.0);
    assert_eq!(encoded_long[1], 0x33, "14B payload → type 0x33");
}

#[test]
fn test_beast_byte_stuffing() {
    use readsb::net::protocols::beast;
    let data = [0x00, 0x1a, 0x84, 0x1a, 0xc3, 0xb3, 0x1d];
    let encoded = beast::encode_beast_output(&data, 0.0);

    assert_eq!(encoded.len(), 18, "Expected 9 header + 9 stuffed payload (7 raw + 2 stuffing)");
    // Frame: 0x1a(1) + 0x32(1) + 6x0 + 0xff = 9 header
    // Stuffed: 0x00 + 0x1a 0x1a + 0x84 + 0x1a 0x1a + 0xc3 + 0xb3 + 0x1d = 9 payload
    assert_eq!(encoded[9], 0x00);
    assert_eq!(encoded[10], 0x1a);
    assert_eq!(encoded[11], 0x1a); // stuffed copy
    assert_eq!(encoded[12], 0x84);
    assert_eq!(encoded[13], 0x1a);
    assert_eq!(encoded[14], 0x1a); // stuffed copy
    assert_eq!(encoded[15], 0xc3);
    assert_eq!(encoded[16], 0xb3);
    assert_eq!(encoded[17], 0x1d);
}

#[test]
fn test_beast_standard_format() {
    use readsb::net::protocols::beast;
    let data = [
        0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71,
        0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
    ];
    let encoded = beast::encode_beast_output(&data, 0.0);

    assert_eq!(encoded[0], 0x1a, "First byte must be 0x1a (frame start)");
    assert_eq!(encoded[1], 0x33, "14B payload → type 0x33 (long frame)");
    for i in 2..8 {
        assert_eq!(encoded[i], 0x00, "Timestamp bytes {i} must be zero");
    }
    assert_eq!(encoded[8], 0xff, "RSSI byte must be 0xff (signal=0 sentinel)");
    assert_eq!(&encoded[9..23], &data, "Payload must be verbatim at bytes 9-22");
    assert_eq!(encoded.len(), 23, "Frame length must be 1+1+6+1+14 = 23");
}
