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
fn test_network_server_per_port_channels() {
    use readsb::net::server::{NetworkServer, InputParser};

    let (server, beast_rx, hex_rx, sbs_rx) = NetworkServer::new(&[
        ("0.0.0.0:0", InputParser::Beast),
        ("0.0.0.0:0", InputParser::Hex),
        ("0.0.0.0:0", InputParser::Sbs),
    ]);

    assert!(!beast_rx.is_closed(), "Beast receiver must be open");
    assert!(!hex_rx.is_closed(), "Hex receiver must be open");
    assert!(!sbs_rx.is_closed(), "SBS receiver must be open");

    assert!(server.beast_tx.send(vec![0x1a, 0x32]).is_ok(), "Beast send must succeed");
    assert!(server.hex_tx.send(b"*8D48;\n".to_vec()).is_ok(), "Hex send must succeed");
    assert!(server.sbs_tx.send(b"MSG,7,...".to_vec()).is_ok(), "SBS send must succeed");
}

#[test]
fn test_sbs_encode_position() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.lat = 51.5;
    aircraft.lon = -0.5;
    aircraft.baro_alt = 35000;
    aircraft.gs = 220.0;
    aircraft.track = 45.0;
    aircraft.baro_rate = 0;
    aircraft.position_valid.update(DataSource::Adsb, now_ms);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,3,1,1,A43EA2,1,"), "MSG,3 must contain ICAO");
    assert!(output.contains(",35000,"), "MSG,3 must contain altitude 35000");

    let stale = now_ms + 120_000;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,3"), "MSG,3 must NOT be emitted when position is stale");
}

#[test]
fn test_sbs_encode_velocity() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.gs = 220.0;
    aircraft.track = 45.0;
    aircraft.baro_rate = 0;
    aircraft.gs_valid.update(DataSource::Adsb, now_ms);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,4,1,1,A43EA2,1,"), "MSG,4 must contain ICAO");
    assert!(output.contains(",220,"), "MSG,4 must contain ground speed 220");
    assert!(output.contains(",45,"), "MSG,4 must contain track 45");
    assert!(output.contains(",0,"), "MSG,4 must contain vertical rate 0");

    let stale = now_ms + 120_000;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,4"), "MSG,4 must NOT be emitted when velocity is stale");
}

#[test]
fn test_sbs_encode_callsign() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.callsign = "BAW123".to_string();
    aircraft.callsign_valid.update(DataSource::Adsb, now_ms);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,1,1,1,A43EA2,1,"), "MSG,1 must contain ICAO A43EA2");
    assert!(output.contains(",BAW123,"), "MSG,1 must contain callsign BAW123");

    let stale = now_ms + 120_000;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,1"), "MSG,1 must NOT be emitted when callsign is stale");
}

#[test]
fn test_sbs_encode_altitude() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};

    let mut aircraft = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1716300000000);
    aircraft.baro_alt = 35000;
    aircraft.baro_alt_valid.update(DataSource::ModeAc, 1716300000000);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300000000);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,5,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000,,35000,,,,,,,,,,,"),
        "MSG,5 must be emitted with altitude=35000 when valid");

    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300120000);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,5"), "MSG,5 must NOT be emitted when altitude is stale");
}

#[test]
fn test_sbs_encode_icao_signal() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::AddrType;

    let aircraft = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1716300000000);
    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300000000);
    let output = String::from_utf8(encoded).unwrap();

    assert!(output.contains("MSG,7,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"),
        "MSG,7 must contain ICAO and timestamp");
    assert!(output.contains("MSG,8,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"),
        "MSG,8 must contain ICAO and timestamp");
    assert!(output.ends_with("\r\n"), "Must end with CRLF");
    assert_eq!(output.lines().count(), 2, "Expected exactly 2 MSG lines");
}

#[test]
fn test_hex_encode_output() {
    use readsb::net::protocols::hex;

    let encoded = hex::encode_hex_output(&[0x8D, 0x48, 0x40, 0xD6]);
    assert_eq!(encoded, b"*8D4840D6;\n", "4-byte payload");

    let encoded_empty = hex::encode_hex_output(&[]);
    assert_eq!(encoded_empty, b"*;\n", "empty payload");

    let encoded_wide = hex::encode_hex_output(&[0x00, 0xFF, 0x0A]);
    assert_eq!(encoded_wide, b"*00FF0A;\n", "zero, max, newline bytes");
}

#[test]
fn test_beast_rssi_encoding() {
    use readsb::net::protocols::beast;
    let payload = [0x1A, 0x2B, 0x3C, 0x4D];

    let with_signal = beast::encode_beast_output(&payload, 5000.0);
    assert_eq!(with_signal[8], 19, "RSSI for signal=5000 should be 19");

    let with_zero = beast::encode_beast_output(&payload, 0.0);
    assert_eq!(with_zero[8], 0xff, "RSSI for signal=0 should be 0xff sentinel");

    let with_negative = beast::encode_beast_output(&payload, -1.0);
    assert_eq!(with_negative[8], 0xff, "RSSI for signal=-1 should be 0xff sentinel");

    let with_moderate = beast::encode_beast_output(&payload, 50000.0);
    assert_eq!(with_moderate[8], 195, "RSSI for signal=50000 should be 195");

    let with_clamped = beast::encode_beast_output(&payload, 100000.0);
    assert_eq!(with_clamped[8], 0xff, "RSSI for signal=100000 should be clamped to 0xff");

    let edge = beast::encode_beast_output(&payload, 65536.0);
    assert_eq!(edge[8], 255, "RSSI for signal=65536 should be 255 (max)");
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

#[test]
fn test_destuff_beast() {
    use readsb::net::protocols::beast;

    let input = [0x00, 0x84, 0xc3, 0xb3, 0x1d];
    let output = beast::destuff_beast(&input);
    assert_eq!(output, input.to_vec(), "No 0x1a bytes → unchanged");

    let input = [0x00, 0x1a, 0x1a, 0x84];
    let output = beast::destuff_beast(&input);
    assert_eq!(output, vec![0x00, 0x1a, 0x84], "0x1a 0x1a → 0x1a");

    let input = [0x00, 0x1a, 0x1a, 0x84, 0x1a, 0x1a, 0xc3];
    let output = beast::destuff_beast(&input);
    assert_eq!(output, vec![0x00, 0x1a, 0x84, 0x1a, 0xc3], "Multiple stuffed");

    let input = [0x1a, 0x84, 0x1a, 0xc3];
    let output = beast::destuff_beast(&input);
    assert_eq!(output, input.to_vec(), "0x1a followed by non-0x1a → unchanged");

    let output = beast::destuff_beast(&[]);
    assert_eq!(output, vec![] as Vec<u8>, "Empty → empty");
}

#[test]
fn test_beast_frame_struct() {
    use readsb::net::protocols::beast::BeastFrame;
    let frame = BeastFrame {
        timestamp: 1716300000000,
        frame_type: 0x33,
        payload: vec![0x8D, 0x48, 0x40],
        rssi: 0xff,
    };
    assert_eq!(frame.frame_type, 0x33);
    assert_eq!(frame.rssi, 0xff);
    assert_eq!(frame.timestamp, 1716300000000);
}
