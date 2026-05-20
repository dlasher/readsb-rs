use readsb::modes::comm_b::CommBFormat;

#[test]
fn test_decode_comm_b_empty() {
    let mb = [0x00u8; 7];
    let format = readsb::modes::comm_b::decode_comm_b(&mb);
    assert_eq!(format, CommBFormat::EmptyResponse);
}

#[test]
fn test_decode_comm_b_aircraft_ident() {
    let mut mb = [0x00u8; 7];
    mb[0] = 0x10; // BDS 1,0
    let format = readsb::modes::comm_b::decode_comm_b(&mb);
    assert_eq!(format, CommBFormat::AircraftIdent);
}

#[test]
fn test_decode_comm_b_acas() {
    let mut mb = [0x00u8; 7];
    mb[0] = 0x30; // BDS 3,0
    mb[1] = 0x01; // Non-empty = ACAS RA
    let format = readsb::modes::comm_b::decode_comm_b(&mb);
    assert_eq!(format, CommBFormat::AcasRA);
}

#[test]
fn test_detect_mode_a_basic() {
    let samples = [40000u16, 100, 100, 40000, 100, 100, 100, 100,
                   100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100];
    let result = readsb::modes::mode_ac::detect_mode_a(&samples);
    assert!(result.is_some(), "Should detect Mode A preamble");
}

#[test]
fn test_detect_mode_a_no_preamble() {
    let samples = [100u16; 20]; // All low = no preamble
    assert!(readsb::modes::mode_ac::detect_mode_a(&samples).is_none());
}

#[test]
fn test_mode_a_to_mode_c() {
    // Mode A value that encodes to a valid altitude
    // 0x2100 should produce a valid Mode C altitude
    let alt = readsb::modes::mode_ac::mode_a_to_mode_c(0x2100);
    assert_ne!(alt, -9999, "Should decode to valid altitude");
}
