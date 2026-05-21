use readsb::tracking::{Aircraft, AircraftRegistry};
use readsb::types::{AddrType, INVALID_ALTITUDE};
use std::sync::Arc;

#[test]
fn test_aircraft_creation() {
    let a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    assert_eq!(a.addr, 0x4840D6);
    assert_eq!(a.seen, 1000);
    assert_eq!(a.baro_alt, INVALID_ALTITUDE);
}

#[test]
fn test_aircraft_signal() {
    let mut a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    a.add_signal(0.5, 1000);
    a.add_signal(0.6, 1001);
    let db = a.get_signal_db();
    assert!(db.is_finite());
    // With these levels, dB value will be negative (log10 of <1 values)
    assert!(db < 0.0);
}

#[test]
fn test_registry_get_or_create() {
    let reg = AircraftRegistry::new();
    let a1 = reg.get_or_create(0x4840D6, 1000, AddrType::AdsbIcao);
    let a2 = reg.get_or_create(0x4840D6, 2000, AddrType::AdsbIcao);
    assert!(Arc::ptr_eq(&a1, &a2));
}

#[test]
fn test_registry_remove_stale() {
    let reg = AircraftRegistry::new();
    reg.get_or_create(0x4840D6, 1000, AddrType::AdsbIcao);
    reg.get_or_create(0x123456, 1000, AddrType::AdsbIcao);
    let removed = reg.remove_stale(31000, 30000);
    assert_eq!(removed, 2);
    assert_eq!(reg.len(), 0);
}

#[test]
fn test_registry_concurrent_access() {
    use std::thread;
    let reg = Arc::new(AircraftRegistry::new());
    let mut handles = vec![];
    for i in 0..10 {
        let reg = reg.clone();
        handles.push(thread::spawn(move || {
            reg.get_or_create(0x4840D6 + i, 1000, AddrType::AdsbIcao);
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(reg.len(), 10);
}

#[test]
fn test_registry_iter_aircraft_empty() {
    let reg = AircraftRegistry::new();
    let result = reg.iter_aircraft();
    assert!(result.is_empty(), "Empty registry should return empty vec");
}

#[test]
fn test_registry_iter_aircraft_returns_all() {
    let reg = AircraftRegistry::new();
    reg.get_or_create(0x4840D6, 1000, AddrType::AdsbIcao);
    reg.get_or_create(0x123456, 1000, AddrType::AdsbIcao);
    let result = reg.iter_aircraft();
    assert_eq!(result.len(), 2, "Should return both aircraft");
    let addrs: Vec<u32> = result.iter().map(|a| a.addr).collect();
    assert!(addrs.contains(&0x4840D6));
    assert!(addrs.contains(&0x123456));
}
