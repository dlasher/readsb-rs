use readsb::stats::Stats;

#[test]
fn test_stats_defaults() {
    let s = Stats::new();
    assert_eq!(s.messages_total, 0);
    assert_eq!(s.demod_preambles, 0);
    assert_eq!(s.demod_accepted, [0, 0, 0]);
    assert_eq!(s.cpr_global_ok, 0);
    assert_eq!(s.samples_processed, 0);
    assert_eq!(s.network_bytes_in, 0);
    assert_eq!(s.network_bytes_out, 0);
    // distance_min should be f64::MAX
    assert!(s.distance_min > 1e30);
    assert_eq!(s.distance_max, 0.0);
}

#[test]
fn test_stats_impl_default() {
    let s: Stats = Default::default();
    assert_eq!(s.messages_total, 0);
}
