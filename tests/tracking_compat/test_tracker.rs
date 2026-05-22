use readsb::tracking::Tracker;
use readsb::types::{
    AddrType, DataSource, Emergency, MessageAccuracy, ModesMessage, NavState,
};

#[test]
fn test_tracker_creates_aircraft() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        ..Default::default()
    };

    let result = tracker.update_from_message(&mm, 1000);
    assert!(result.is_some());
    assert_eq!(tracker.registry.len(), 1);
}

#[test]
fn test_tracker_ignores_invalid_address() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0,
        ..Default::default()
    };
    assert!(tracker.update_from_message(&mm, 1000).is_none());
    assert_eq!(tracker.registry.len(), 0);
}

#[test]
fn test_tracker_updates_altitude_baro() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        baro_alt: 35000,
        baro_alt_valid: true,
        ..Default::default()
    };

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
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        baro_alt: 35000,
        baro_alt_valid: true,
        geom_alt: 36000,
        geom_alt_valid: true,
        ..Default::default()
    };

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
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        callsign_valid: true,
        callsign: *b"BAW123\0\0\0\0\0\0\0\0\0\0",
        ..Default::default()
    };

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.callsign, "BAW123");
    assert_eq!(a_ref.callsign_valid.source, DataSource::Adsb);
}

#[test]
fn test_tracker_updates_velocity() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        gs: 450.0,
        gs_valid: true,
        ias: 280,
        ias_valid: true,
        tas: 460,
        tas_valid: true,
        mach: 0.82,
        mach_valid: true,
        ..Default::default()
    };

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
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        squawk_valid: true,
        squawk_hex: 0x1234,
        ..Default::default()
    };

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.squawk, 0x1234);
    assert_eq!(a_ref.squawk_valid.source, DataSource::Adsb);
}

#[test]
fn test_tracker_updates_emergency() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        emergency: Emergency::General,
        ..Default::default()
    };

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.emergency, Emergency::General);
}

#[test]
fn test_tracker_updates_nav() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        nav: NavState {
            mcp_altitude_valid: true,
            mcp_altitude: 10000,
            fms_altitude_valid: true,
            fms_altitude: 9500,
            qnh_valid: true,
            qnh: 1013.25,
            ..Default::default()
        },
        ..Default::default()
    };

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
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        accuracy: MessageAccuracy {
            nac_p: 8,
            ..Default::default()
        },
        ..Default::default()
    };

    let result = tracker.update_from_message(&mm, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.pos_nic, 8);
    assert_eq!(a_ref.pos_rc, 8);
}

#[test]
fn test_tracker_remove_stale() {
    let tracker = Tracker::new();
    let mm1 = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        ..Default::default()
    };
    tracker.update_from_message(&mm1, 1000);

    let mm2 = ModesMessage {
        addr: 0x123456,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        ..Default::default()
    };
    tracker.update_from_message(&mm2, 1000);

    assert_eq!(tracker.registry.len(), 2);

    // At 61s after creation, aircraft should NOT be expired with 300s TRACK_EXPIRE
    let removed = tracker.remove_stale(61000);
    assert_eq!(removed, 0);
    assert_eq!(tracker.registry.len(), 2);

    // At 301001ms (just past 300s expire) they should be removed
    let removed = tracker.remove_stale(301001);
    assert_eq!(removed, 2);
    assert_eq!(tracker.registry.len(), 0);
}

#[test]
fn test_tracker_seen_updates_on_message() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        ..Default::default()
    };

    // First message at t=1000
    tracker.update_from_message(&mm, 1000);

    // Second message at t=5000 — seen should update to 5000
    tracker.update_from_message(&mm, 5000);
    let a = tracker.registry.get(0x4840D6).unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.seen, 5000);
}

#[test]
fn test_tracker_cpr_pairing() {
    let tracker = Tracker::new();

    // Send even frame first
    let even = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        cpr_valid: true,
        cpr_odd: false,
        cpr_lat: 12345,
        cpr_lon: 67890,
        ..Default::default()
    };

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
    let odd = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        cpr_valid: true,
        cpr_odd: true,
        cpr_lat: 12400,
        cpr_lon: 67800,
        ..Default::default()
    };

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

#[test]
fn test_tracker_update_squawk_only() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        ..Default::default()
    };
    tracker.update_from_message(&mm, 1000);

    tracker.update_squawk_only(0x4840D6, 0o1234, 2000);
    let a = tracker.registry.get(0x4840D6).unwrap();
    let a_ref = a.read().unwrap();
    assert_eq!(a_ref.squawk, 0o1234);
}

#[test]
fn test_tracker_signal_level() {
    let tracker = Tracker::new();
    let mm = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        signal_level: 5000.0,
        ..Default::default()
    };

    tracker.update_from_message(&mm, 1000);
    let a = tracker.registry.get(0x4840D6).unwrap();
    let a_ref = a.read().unwrap();
    assert!(a_ref.get_signal_db() > 0.0);
}

#[test]
fn test_tracker_cpr_relative_decode() {
    let mut tracker = Tracker::new();
    tracker.user_lat = 48.0;
    tracker.user_lon = 10.0;

    let even = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        cpr_valid: true,
        cpr_odd: false,
        cpr_lat: 12345,
        cpr_lon: 67890,
        ..Default::default()
    };

    let result = tracker.update_from_message(&even, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    // Single even frame should resolve position via relative decode
    assert!(a_ref.lat != 0.0, "lat should be resolved");
    assert!(a_ref.lon != 0.0, "lon should be resolved");
}

#[test]
fn test_tracker_range_filter() {
    let mut tracker = Tracker::new();
    tracker.user_lat = 48.0;
    tracker.user_lon = 10.0;
    tracker.max_range = 1.0; // 1 meter — impossibly small

    let even = ModesMessage {
        addr: 0x4840D6,
        addrtype: AddrType::AdsbIcao,
        source: DataSource::Adsb,
        cpr_valid: true,
        cpr_odd: false,
        cpr_lat: 12345,
        cpr_lon: 67890,
        ..Default::default()
    };

    let result = tracker.update_from_message(&even, 1000);
    let a = result.unwrap();
    let a_ref = a.read().unwrap();
    // Position should be filtered out by range check
    assert_eq!(a_ref.lat, 0.0, "lat should be filtered by range");
    assert_eq!(a_ref.lon, 0.0, "lon should be filtered by range");
}
