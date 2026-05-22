use readsb::output::{AircraftJson, globe_index, bincraft::BinCraft};
use readsb::tracking::Aircraft;
use readsb::types::*;

#[test]
fn test_aircraft_json_serialization() {
    let mut a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    a.callsign = "BAW123".to_string();
    a.baro_alt = 35000;
    a.gs = 450.0;
    a.track = 270.0;
    a.lat = 51.5;
    a.lon = -0.5;
    a.squawk = 0o1234;
    a.messages = 42;

    let j = AircraftJson::from_aircraft(&a, 2000);
    assert_eq!(j.hex, "4840D6");
    assert_eq!(j.flight, Some("BAW123".to_string()));
    assert_eq!(j.alt_baro, Some(35000));
    assert_eq!(j.gs, Some(450.0));
    assert_eq!(j.lat, Some(51.5));
    assert_eq!(j.lon, Some(-0.5));
    assert_eq!(j.squawk, Some("1234".to_string()));
    assert_eq!(j.messages, 42);
    assert_eq!(j.seen, 1000);
}

#[test]
fn test_aircraft_json_empty_fields() {
    let a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    let j = AircraftJson::from_aircraft(&a, 1000);
    assert_eq!(j.flight, None);
    assert_eq!(j.alt_baro, None);
    assert_eq!(j.gs, None);
    assert_eq!(j.lat, None);
    assert_eq!(j.lon, None);
    assert_eq!(j.squawk, None);
    assert_eq!(j.category, None);
}

#[test]
fn test_globe_index_bounds() {
    // At equator/prime meridian
    let idx1 = globe_index(0.0, 0.0);
    assert!(idx1 >= 1000);
    // At a different location
    let idx2 = globe_index(51.5, -0.5);
    assert_ne!(idx1, idx2);
    // Edge of world
    let idx3 = globe_index(-89.0, -179.0);
    assert!(idx3 >= 1000);
}

#[test]
fn test_bincraft_memory_layout() {
    assert_eq!(std::mem::size_of::<BinCraft>(), 112, "BinCraft must be exactly 112 bytes");
    // Verify using field offset checks
    let _bc = BinCraft {
        hex: 0x4840D6,
        seen: 1000,
        lon: -50000,
        lat: 51500000,
        baro_rate: 0, geom_rate: 0,
        baro_alt: 350, geom_alt: 360,
        nav_alt_mcp: 0, nav_alt_fms: 0,
        nav_qnh: 0, nav_heading: 0,
        squawk: 0x1234, gs: 450, mach: 0,
        roll: 0, track: 2700, track_rate: 0,
        mag_heading: 0, true_heading: 0,
        wind_dir: 0, wind_speed: 0,
        oat: 0, tat: 0,
        tas: 0, ias: 0,
        pos_rc: 0, messages: 42,
        category: 0, pos_nic: 0,
        nav_modes: 0, emergency: 0,
        airground: 0, nav_alt_src: 0,
        sil_type: 0, adsb_version: 0,
        callsign: [b'B', b'A', b'W', b'1', b'2', b'3', b' ', b' '],
        _pad: [0; 32],
    };
    // Verify member fields are accessible (smoke test against struct changes)
    assert_eq!(std::mem::size_of::<BinCraft>(), 112, "BinCraft must be exactly 112 bytes");
}
