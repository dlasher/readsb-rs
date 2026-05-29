use readsb::crc::{modes_checksum, CrcFixEngine};
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
fn test_me_field_extraction() {
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&bytes, 112, &engine, 0.0).unwrap();
    assert_eq!(result.message.me[0], 0x20);
    assert_eq!(result.message.me[1], 0x2C);
    assert_eq!(result.message.me[2], 0xC3);
    assert_eq!(result.message.me[3], 0x71);
    assert_eq!(result.message.me[4], 0xC3);
    assert_eq!(result.message.me[5], 0x2C);
    assert_eq!(result.message.me[6], 0xE0);
}

#[test]
fn test_cpr_decoded_set_on_airborne_position() {
    // Build a DF17 Airborne Position (metype=11) with known CPR values and altitude
    // Per ICAO Annex 10 / ADS-B standard:
    //   bits 9-20 = altitude in me[1] + me[2] top 4 bits
    //   bit 22 = F flag in me[2] bit 2
    //   bits 23-39 = CPR lat in me[2] bits 1-0, me[3], me[4] bits 7-1
    //   bits 40-56 = CPR lon in me[4] bit 0, me[5], me[6]
    // Altitude: 12-bit field with Q-bit at bit 4. Q=1 -> n*25-1000.
    // alt_raw = 0x378 -> Q=1, n=440 -> 10000ft
    // CPR even: lat=80536, lon=9432 (from cpr_compat test)
    let mut msg = [0u8; 14];
    msg[0] = 0x8D; // DF17 + CA
    msg[1..4].copy_from_slice(&[0x48, 0x40, 0xD6]); // ICAO
    msg[4] = 0x58; // ME[0]: TC=11, SS=0, NICsb=0
    // alt_raw = (me[1] << 4) | (me[2] >> 4) = 0x370 | 0x08 = 0x378
    msg[5] = 0x37; // me[1]: altitude bits 11-4
    msg[6] = 0x82; // me[2]: altitude[3:0]=8<<4, F=0, CPR_lat[16:15]=2
    msg[7] = 0x75; // me[3]: CPR_lat bits 14-7
    msg[8] = 0x30; // me[4]: CPR_lat bits 6-0 <<1 | CPR_lon bit 16
    msg[9] = 0x24; // me[5]: CPR_lon bits 15-8
    msg[10] = 0xD8; // me[6]: CPR_lon bits 7-0
    // Compute and embed CRC (PI field in bytes 11-13)
    let pi = modes_checksum(&msg, 112);
    msg[11] = (pi >> 16) as u8;
    msg[12] = (pi >> 8) as u8;
    msg[13] = pi as u8;
    assert_eq!(modes_checksum(&msg, 112), 0, "CRC must be valid");

    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&msg, 112, &engine, 0.0).unwrap();
    assert!(result.crc_ok, "CRC must pass");
    assert_eq!(result.message.metype, 11, "metype should be 11");
    assert!(result.message.cpr_valid, "Airborne position should have valid CPR");
    assert!(result.message.cpr_decoded, "cpr_decoded should be true for airborne position");
    assert_eq!(result.message.cpr_lat, 80536, "CPR lat should match");
    assert_eq!(result.message.cpr_lon, 9432, "CPR lon should match");
    assert!(result.message.baro_alt_valid, "Baro alt should be valid");
    assert_eq!(result.message.baro_alt, 10000, "Baro alt should be 10000ft");
}

#[test]
fn test_aircraft_identification_decodes_callsign() {
    // 8D4840D6202CC371C32CE0576098 is TC=4 (Aircraft Identification)
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&bytes, 112, &engine, 0.0).unwrap();
    assert!(result.message.callsign_valid, "TC=4 should produce callsign");
    let cs = String::from_utf8_lossy(&result.message.callsign).trim_end_matches('\0').to_string();
    assert!(!cs.is_empty(), "Callsign should not be empty");
    assert_eq!(cs, "KLM1023", "Fixture should decode to 'KLM1023'");
}

#[test]
fn test_airborne_velocity_decodes_heading() {
    // Build a DF17 Airborne Velocity (metype=19, mesub=1)
    // EW velocity = 200kt East, NS velocity = 346kt North → heading ~30° at ~400kt GS
    // C getbits uses MSB-first byte ordering (b7=C1, b6=C2, ...)
    let mut msg = [0u8; 14];
    msg[0] = 0x8D;
    msg[1..4].copy_from_slice(&[0x48, 0x40, 0xD6]);
    msg[4] = 0x99; // ME[0]: metype=19, mesub=1 (airborne velocity, subsonic)
    // EW raw=201: me[1] b1,b0 = 00 (top 2 bits of 201), me[2] b7..b0 = 0xC9 (201)
    // EW direction: me[1] b2 = 0 (West, positive)
    // NS raw=347: me[3] b6..b0 = 0x2B (top 7 bits), me[4] b7..b5 = 3 (bottom 3 bits << 5)
    // NS direction: me[3] b7 = 0 (North, positive)
    msg[5] = 0x00; // me[1]: ew_sign(0) at b2, ew_raw top 2 bits at b1,b0
    msg[6] = 0xC9; // me[2]: ew_raw bottom 8 bits = 201
    msg[7] = 0x2B; // me[3]: ns_sign(0) at b7, ns_raw top 7 bits = 0x2B
    msg[8] = 0x60; // me[4]: ns_raw bottom 3 bits (3) at b7..b5
    msg[9] = 0;    // me[5]: VR + spare
    msg[10] = 0;   // me[6]: VR
    let pi = modes_checksum(&msg, 112);
    msg[11] = (pi >> 16) as u8;
    msg[12] = (pi >> 8) as u8;
    msg[13] = pi as u8;
    assert_eq!(modes_checksum(&msg, 112), 0);
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&msg, 112, &engine, 0.0).unwrap();
    assert!(result.crc_ok);
    assert!(result.message.gs_valid, "GS should be valid");
    assert!(result.message.gs > 0.0, "GS should be > 0");
    assert!(result.message.track_valid, "Track should be valid");
    assert!(result.message.track > 0.0, "Heading should be > 0");
    // GS = sqrt(200² + 346²) ≈ 400kt, heading = atan2(200, 346) ≈ 30°
    assert!((result.message.gs - 400.0).abs() < 50.0, "GS should be ~400kt: {}", result.message.gs);
    assert!((result.message.track - 30.0).abs() < 5.0, "Heading should be ~30°: {}", result.message.track);
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

#[test]
fn test_df5_squawk_decodes_icao_and_squawk() {
    // DF5 (short frame, 56-bit) uses Address/Parity:
    // The CRC syndrome IS the sender's ICAO address.
    // modes_checksum returns ICAO for correct DF5 messages (non-zero).
    //
    // Current bug: mm.addr is never extracted from the CRC for DF5/21,
    // and crc_ok is set to false for all non-zero CRC values, filtering them out.
    use readsb::crc::modes_checksum;

    // Construct a DF5 message with ID=0x1000 (squawk=0x2000),
    // ICAO=0x4840D6, and correct CRC parity.
    let mut msg = [0u8; 7];
    msg[0] = (5 << 3) | 5; // DF=5, CA=5 (0x2D)
    // ID field bits 12-0: msg[2] low 5 bits | msg[3] all bits
    // ID=0x1000 → msg[2]=0x10, msg[3]=0x00
    msg[2] = 0x10; // ID bits 12-8 = 0x10
    msg[3] = 0x00; // ID bits 7-0 = 0x00

    // Compute CRC parity for ICAO address 0x4840D6
    // For Address/Parity frames, parity = CRC(msg[0..3]) XOR ICAO
    let crc_header = modes_checksum(&msg, 56); // CRC over bytes 0-3 (bytes 4-6 are zero)
    let icao = 0x4840D6u32;
    let parity = crc_header ^ icao;
    msg[4] = (parity >> 16) as u8;
    msg[5] = (parity >> 8) as u8;
    msg[6] = parity as u8;

    // Verify: for Address/Parity frame, modes_checksum returns ICAO
    let crc = modes_checksum(&msg, 56);
    assert_eq!(crc, icao, "DF5 CRC must equal ICAO address (Address/Parity encoding)");

    let engine = CrcFixEngine::new(56);
    let result = parse_modes_message(&msg, 56, &engine, 0.0).unwrap();

    // Address/Parity message: CRC == ICAO, so the message is correct but non-zero CRC
    // crc_ok should be true because this is expected behavior for Address/Parity frames
    assert!(result.crc_ok, "DF5 Address/Parity: CRC equals ICAO, should be accepted");

    // Squawk must be decoded
    assert!(result.message.squawk_valid, "Squawk should be valid");
    // ID=0x1000 → decode: (0x1000 & 0x1F00)<<1 = 0x2000
    assert_eq!(result.message.squawk_hex, 0x2000,
        "Squawk should be 0x2000, got 0x{:04X}",
        result.message.squawk_hex);

    // The ICAO must be extracted from the CRC parity
    assert_eq!(result.message.addr, 0x4840D6,
        "DF5 ICAO address should be extracted from CRC parity, got 0x{:06X}",
        result.message.addr);
}

#[test]
fn test_tc28_aircraft_status_decodes_squawk() {
    // TC=28, mesub=1 (Aircraft Status) contains squawk in ME bits 12-24
    // ME[1] bits 7-5 = emergency, bits 4-0 | ME[2] bits 7-0 = ID13 squawk field
    let mut msg = [0u8; 14];
    msg[0] = 0x8D; // DF17 + CA
    msg[1..4].copy_from_slice(&[0x48, 0x40, 0xD6]); // ICAO
    msg[4] = 0xE1; // ME[0]: TC=28, mesub=1
    // Squawk 0x2000 = Gillham: ID13 bits 12-0
    // ID13 = 0x1000 → C1=1, rest=0 → decode_id13 → 0x0010
    msg[5] = 0x10; // ME[1]: emergency=0, ID13 bits 12-8 = 0x10
    msg[6] = 0x00; // ME[2]: ID13 bits 7-0 = 0x00
    let pi = modes_checksum(&msg, 112);
    msg[11] = (pi >> 16) as u8;
    msg[12] = (pi >> 8) as u8;
    msg[13] = pi as u8;
    assert_eq!(modes_checksum(&msg, 112), 0);

    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&msg, 112, &engine, 0.0).unwrap();
    assert!(result.crc_ok);
    assert!(result.message.squawk_valid, "TC=28 mesub=1 should have squawk");
    assert_eq!(result.message.squawk_hex, 0x0010, "Squawk should be 0x0010");
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) { return None; }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok()).collect()
}
