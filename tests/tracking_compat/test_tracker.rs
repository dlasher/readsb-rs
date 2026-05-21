use readsb::tracking::Tracker;
use readsb::types::{ModesMessage, DataSource, AddrType, Emergency};

#[test]
fn test_tracker_creates_aircraft() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;

    let result = tracker.update_from_message(&mm, 1000);
    assert!(result.is_some());
    assert_eq!(tracker.registry.len(), 1);
}

#[test]
fn test_tracker_ignores_invalid_address() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0;
    assert!(tracker.update_from_message(&mm, 1000).is_none());
    assert_eq!(tracker.registry.len(), 0);
}

#[test]
fn test_tracker_updates_altitude_baro() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.baro_alt = 35000;
    mm.baro_alt_valid = true;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.baro_alt, 35000);
    assert_eq!(a_ref.baro_alt_valid.source, DataSource::Adsb);
    assert!(!a_ref.baro_alt_valid.stale);
}

#[test]
fn test_tracker_updates_altitude_geom() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.baro_alt = 35000;
    mm.baro_alt_valid = true;
    mm.geom_alt = 36000;
    mm.geom_alt_valid = true;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.baro_alt, 35000);
    assert_eq!(a_ref.geom_alt, 36000);
    assert_eq!(a_ref.geom_delta, 1000);
}

#[test]
fn test_tracker_updates_callsign() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.callsign_valid = true;
    let cs: &[u8] = b"BAW123\0\0\0\0\0\0\0\0\0\0";
    mm.callsign.copy_from_slice(cs);

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.callsign, "BAW123");
    assert_eq!(a_ref.callsign_valid.source, DataSource::Adsb);
}

#[test]
fn test_tracker_updates_velocity() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.gs = 450.0;
    mm.gs_valid = true;
    mm.ias_valid = true;
    mm.ias = 280;
    mm.tas_valid = true;
    mm.tas = 460;
    mm.mach_valid = true;
    mm.mach = 0.82;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.gs, 450.0);
    assert_eq!(a_ref.gs_valid.source, DataSource::Adsb);
    assert_eq!(a_ref.ias, 280);
    assert_eq!(a_ref.tas, 460);
    assert!((a_ref.mach - 0.82).abs() < 0.001);
}

#[test]
fn test_tracker_updates_squawk() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.squawk_valid = true;
    mm.squawk_hex = 0x1234;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.squawk, 0x1234);
    assert_eq!(a_ref.squawk_valid.source, DataSource::Adsb);
}

#[test]
fn test_tracker_updates_emergency() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.emergency = Emergency::General;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.emergency, Emergency::General);
}

#[test]
fn test_tracker_updates_nav() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.nav.mcp_altitude_valid = true;
    mm.nav.mcp_altitude = 10000;
    mm.nav.fms_altitude_valid = true;
    mm.nav.fms_altitude = 9500;
    mm.nav.qnh_valid = true;
    mm.nav.qnh = 1013.25;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.nav_altitude_mcp, 10000);
    assert_eq!(a_ref.nav_altitude_fms, 9500);
    assert!((a_ref.nav_qnh - 1013.25).abs() < 0.01);
}

#[test]
fn test_tracker_updates_accuracy() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6;
    mm.addrtype = AddrType::AdsbIcao;
    mm.source = DataSource::Adsb;
    mm.accuracy.nac_p = 8;

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.pos_nic, 8);
    assert_eq!(a_ref.pos_rc, 8);
}

#[test]
fn test_tracker_remove_stale() {
    let tracker = Tracker::new();
    let mut mm1 = ModesMessage::default();
    mm1.addr = 0x4840D6;
    mm1.addrtype = AddrType::AdsbIcao;
    mm1.source = DataSource::Adsb;
    tracker.update_from_message(&mm1, 1000);

    let mut mm2 = ModesMessage::default();
    mm2.addr = 0x123456;
    mm2.addrtype = AddrType::AdsbIcao;
    mm2.source = DataSource::Adsb;
    tracker.update_from_message(&mm2, 1000);

    assert_eq!(tracker.registry.len(), 2);

    let removed = tracker.remove_stale(61000);
    assert_eq!(removed, 2);
    assert_eq!(tracker.registry.len(), 0);
}

#[test]
fn test_tracker_cpr_pairing() {
    let tracker = Tracker::new();

    // Send even frame first
    let mut even = ModesMessage::default();
    even.addr = 0x4840D6;
    even.addrtype = AddrType::AdsbIcao;
    even.source = DataSource::Adsb;
    even.cpr_valid = true;
    even.cpr_odd = false;
    even.cpr_lat = 12345;
    even.cpr_lon = 67890;

    let result1 = tracker.update_from_message(&even, 1000);
    {
        let a = result1.unwrap();
        let a_ref = a.read().unwrap();
        // Even frame stored, but not decoded yet (no odd frame pair)
        assert_eq!(a_ref.cpr_even_lat, 12345);
        assert_eq!(a_ref.cpr_even_lon, 67890);
        assert_eq!(a_ref.lat, 0.0);
        assert_eq!(a_ref.lon, 0.0);
    }

    // Send odd frame (should trigger position decode)
    let mut odd = ModesMessage::default();
    odd.addr = 0x4840D6;
    odd.addrtype = AddrType::AdsbIcao;
    odd.source = DataSource::Adsb;
    odd.cpr_valid = true;
    odd.cpr_odd = true;
    odd.cpr_lat = 12400;
    odd.cpr_lon = 67800;

    let result2 = tracker.update_from_message(&odd, 1100);
    {
        let a = result2.unwrap();
        let a_ref = a.read().unwrap();
        // Odd frame stored
        assert_eq!(a_ref.cpr_odd_lat, 12400);
        assert_eq!(a_ref.cpr_odd_lon, 67800);
        // Position should now be decoded
        assert!(a_ref.lat != 0.0);
        assert!(a_ref.lon != 0.0);
        assert!(a_ref.lat.abs() <= 90.0);
        assert!(a_ref.lon.abs() <= 180.0);
        assert!(a_ref.position_valid.source == DataSource::Adsb);
    }
}
