use readsb::cpr::{decode_cpr_airborne, decode_cpr_surface, decode_cpr_relative};

// Tolerance for cross-validation against C reference (1e-6 as in cprtests.c)
const TOL: f64 = 1e-6;

#[test]
fn test_cpr_airborne_basic() {
    let result = decode_cpr_airborne(12345, 67890, 12400, 67800, 0);
    assert!(result.is_some(), "Airborne CPR should decode");
    let (lat, lon) = result.unwrap();
    assert!(lat.abs() <= 90.0, "Latitude out of range: {}", lat);
    assert!(lon.abs() <= 180.0, "Longitude out of range: {}", lon);
}

#[test]
fn test_cpr_surface_basic() {
    let result = decode_cpr_surface(52.0, -0.5, 1000, 2000, 1100, 2100, 0);
    assert!(result.is_some(), "Surface CPR should decode");
    let (lat, lon) = result.unwrap();
    assert!(lat.abs() <= 90.0);
    assert!(lon.abs() <= 180.0);
}

#[test]
fn test_cpr_relative_basic() {
    let result = decode_cpr_relative(52.0, -0.5, 1000, 2000, 0, false);
    assert!(result.is_some(), "Relative CPR should decode");
}

#[test]
fn test_cpr_zero_values() {
    let result = decode_cpr_airborne(0, 0, 0, 0, 0);
    assert!(result.is_none(), "Zero CPR values should fail to decode");
}

// ── Cross-validation tests matching C reference (cprtests.c) ──

#[test]
fn test_cpr_airborne_cross_validate_even() {
    // testCPRGlobalAirborne[0, EVEN]
    let result = decode_cpr_airborne(80536, 9432, 61720, 9192, 0);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686646).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.700156).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_airborne_cross_validate_odd() {
    // testCPRGlobalAirborne[0, ODD]
    let result = decode_cpr_airborne(80536, 9432, 61720, 9192, 1);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686763).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.701294).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_airborne_cross_validate_even_1() {
    // testCPRGlobalAirborne[1, EVEN]
    let result = decode_cpr_airborne(80534, 9413, 61714, 9144, 0);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686554).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.698745).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_airborne_cross_validate_odd_1() {
    // testCPRGlobalAirborne[1, ODD]
    let result = decode_cpr_airborne(80534, 9413, 61714, 9144, 1);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686484).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.697632).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_surface_cross_validate_even() {
    // testCPRGlobalSurface[6, EVEN]: ref(52.00, 0.00)
    let result = decode_cpr_surface(52.00, 0.00, 105730, 9259, 29693, 8997, 0);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 52.209984).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.176601).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_surface_cross_validate_odd() {
    // testCPRGlobalSurface[6, ODD]: ref(52.00, 0.00)
    let result = decode_cpr_surface(52.00, 0.00, 105730, 9259, 29693, 8997, 1);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 52.209976).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.176507).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_relative_cross_validate_even() {
    // testCPRRelative[0]: ref(52.00, 0.00), cpr(80536, 9432), fflag=0, surface=0
    let result = decode_cpr_relative(52.00, 0.00, 80536, 9432, 0, false);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686646).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.700156).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_relative_cross_validate_odd() {
    // testCPRRelative[1]: ref(52.00, 0.00), cpr(61720, 9192), fflag=1, surface=0
    let result = decode_cpr_relative(52.00, 0.00, 61720, 9192, 1, false);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 51.686763).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.701294).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_relative_surface_even() {
    // testCPRRelative[20]: ref(52.00, 0.00), cpr(105730, 9259), fflag=0, surface=1
    let result = decode_cpr_relative(52.00, 0.00, 105730, 9259, 0, true);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 52.209984).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.176601).abs() < TOL, "lon mismatch: {:.10}", lon);
}

#[test]
fn test_cpr_relative_surface_odd() {
    // testCPRRelative[21]: ref(52.00, 0.00), cpr(29693, 8997), fflag=1, surface=1
    let result = decode_cpr_relative(52.00, 0.00, 29693, 8997, 1, true);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!((lat - 52.209976).abs() < TOL, "lat mismatch: {:.10}", lat);
    assert!((lon - 0.176507).abs() < TOL, "lon mismatch: {:.10}", lon);
}
