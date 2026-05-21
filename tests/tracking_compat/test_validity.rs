use readsb::tracking::DataValidity;
use readsb::types::DataSource;

#[test]
fn test_validity_fresh_data() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    assert!(v.is_valid(1000, 60000));
    assert!(!v.stale);
}

#[test]
fn test_validity_stale_data() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    v.update_stale(61001);
    assert!(v.stale);
}

#[test]
fn test_validity_expired() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    assert!(!v.is_valid(61000, 60000));
}

#[test]
fn test_validity_source_priority() {
    let mut v = DataValidity::new();
    v.update(DataSource::ModeS, 1000);
    v.update(DataSource::Adsb, 2000);
    assert_eq!(v.source, DataSource::Adsb);
    v.update(DataSource::ModeS, 3000);
    assert_eq!(v.source, DataSource::Adsb);
    assert_eq!(v.updated, 2000);
}
