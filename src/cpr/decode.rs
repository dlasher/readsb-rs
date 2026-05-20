// CPR Decoding Functions
//
// Matches the algorithm in cpr.c from readsb.
// See: http://www.lll.lu/~edward/edward/adsb/DecodingADSBposition.html

/// Always-positive modulo for integers (matches cprModInt).
fn cpr_mod_int(a: i32, b: i32) -> i32 {
    let r = a % b;
    if r < 0 { r + b } else { r }
}

/// Always-positive modulo for f64 (matches cprModDouble).
fn cpr_mod_double(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r < 0.0 { r + b } else { r }
}

/// NL(lat) — number of longitude zones at a given latitude.
/// Matches cprNLFunction() in cpr.c.
fn cpr_nl(lat: f64) -> i32 {
    let lat = lat.abs();
    if lat < 10.47047130 { return 59; }
    if lat < 14.82817437 { return 58; }
    if lat < 18.18626357 { return 57; }
    if lat < 21.02939493 { return 56; }
    if lat < 23.54504487 { return 55; }
    if lat < 25.82924707 { return 54; }
    if lat < 27.93898710 { return 53; }
    if lat < 29.91135686 { return 52; }
    if lat < 31.77209708 { return 51; }
    if lat < 33.53993436 { return 50; }
    if lat < 35.22899598 { return 49; }
    if lat < 36.85025108 { return 48; }
    if lat < 38.41241892 { return 47; }
    if lat < 39.92256684 { return 46; }
    if lat < 41.38651832 { return 45; }
    if lat < 42.80914012 { return 44; }
    if lat < 44.19454951 { return 43; }
    if lat < 45.54626723 { return 42; }
    if lat < 46.86733252 { return 41; }
    if lat < 48.16039128 { return 40; }
    if lat < 49.42776439 { return 39; }
    if lat < 50.67150166 { return 38; }
    if lat < 51.89342469 { return 37; }
    if lat < 53.09516153 { return 36; }
    if lat < 54.27817472 { return 35; }
    if lat < 55.44378444 { return 34; }
    if lat < 56.59318756 { return 33; }
    if lat < 57.72747354 { return 32; }
    if lat < 58.84763776 { return 31; }
    if lat < 59.95459277 { return 30; }
    if lat < 61.04917774 { return 29; }
    if lat < 62.13216659 { return 28; }
    if lat < 63.20427479 { return 27; }
    if lat < 64.26616523 { return 26; }
    if lat < 65.31845310 { return 25; }
    if lat < 66.36171008 { return 24; }
    if lat < 67.39646774 { return 23; }
    if lat < 68.42322022 { return 22; }
    if lat < 69.44242631 { return 21; }
    if lat < 70.45451075 { return 20; }
    if lat < 71.45986473 { return 19; }
    if lat < 72.45884545 { return 18; }
    if lat < 73.45177442 { return 17; }
    if lat < 74.43893416 { return 16; }
    if lat < 75.42056257 { return 15; }
    if lat < 76.39684391 { return 14; }
    if lat < 77.36789461 { return 13; }
    if lat < 78.33374083 { return 12; }
    if lat < 79.29428225 { return 11; }
    if lat < 80.24923213 { return 10; }
    if lat < 81.19801349 { return 9; }
    if lat < 82.13956981 { return 8; }
    if lat < 83.07199445 { return 7; }
    if lat < 83.99173563 { return 6; }
    if lat < 84.89166191 { return 5; }
    if lat < 85.75541621 { return 4; }
    if lat < 86.53536998 { return 3; }
    if lat < 87.0          { return 2; }
    1
}

/// N(lat, fflag) = NL(lat) - fflag, minimum 1.
/// Matches cprNFunction() in cpr.c.
fn cpr_n_func(lat: f64, fflag: i32) -> i32 {
    let n = cpr_nl(lat) - fflag;
    if n < 1 { 1 } else { n }
}

/// Dlon(lat, fflag, surface) = (surface ? 90 : 360) / N(lat, fflag).
/// Matches cprDlonFunction() in cpr.c.
fn cpr_dlon_function(lat: f64, fflag: i32, surface: bool) -> f64 {
    let divisor = if surface { 90.0 } else { 360.0 };
    divisor / cpr_n_func(lat, fflag) as f64
}

/// Decode airborne CPR position from even/odd frame pair.
/// Matches decodeCPRairborne() in cpr.c.
///
/// Returns `Some((lat, lon))` on success, or `None` if the position
/// cannot be decoded (bad data, zone boundary crossing, etc.).
pub fn decode_cpr_airborne(
    even_cprlat: i32, even_cprlon: i32,
    odd_cprlat: i32, odd_cprlon: i32,
    fflag: i32,
) -> Option<(f64, f64)> {
    const CPR_RES: f64 = 131072.0;

    // Guard: all-zero raw values cannot be resolved
    if even_cprlat == 0 && even_cprlon == 0 && odd_cprlat == 0 && odd_cprlon == 0 {
        return None;
    }

    let dlat_even = 360.0 / 60.0;   // 6.0°
    let dlat_odd  = 360.0 / 59.0;   // ~6.102°

    let lat0 = even_cprlat as f64;
    let lat1 = odd_cprlat  as f64;
    let lon0 = even_cprlon as f64;
    let lon1 = odd_cprlon  as f64;

    // Compute the Latitude Index "j"
    let j = ((59.0 * lat0 - 60.0 * lat1) / CPR_RES + 0.5).floor() as i32;

    let mut rlat0 = dlat_even * (cpr_mod_int(j, 60) as f64 + lat0 / CPR_RES);
    let mut rlat1 = dlat_odd  * (cpr_mod_int(j, 59) as f64 + lat1 / CPR_RES);

    // Normalize latitude
    if rlat0 >= 270.0 { rlat0 -= 360.0; }
    if rlat1 >= 270.0 { rlat1 -= 360.0; }

    // Check latitude range
    if rlat0 < -90.0 || rlat0 > 90.0 || rlat1 < -90.0 || rlat1 > 90.0 {
        return None;
    }

    // Check that both positions are in the same latitude zone
    if cpr_nl(rlat0) != cpr_nl(rlat1) {
        return None;
    }

    // Compute longitude
    let (rlat, mut rlon) = if fflag != 0 {
        // Use odd packet
        let ni = cpr_n_func(rlat1, 1);
        let m = ((lon0 * (cpr_nl(rlat1) - 1) as f64 - lon1 * cpr_nl(rlat1) as f64)
            / CPR_RES + 0.5).floor() as i32;
        let rlon = cpr_dlon_function(rlat1, 1, false)
            * (cpr_mod_int(m, ni) as f64 + lon1 / CPR_RES);
        (rlat1, rlon)
    } else {
        // Use even packet
        let ni = cpr_n_func(rlat0, 0);
        let m = ((lon0 * (cpr_nl(rlat0) - 1) as f64 - lon1 * cpr_nl(rlat0) as f64)
            / CPR_RES + 0.5).floor() as i32;
        let rlon = cpr_dlon_function(rlat0, 0, false)
            * (cpr_mod_int(m, ni) as f64 + lon0 / CPR_RES);
        (rlat0, rlon)
    };

    // Renormalize longitude to -180 .. +180
    rlon -= ((rlon + 180.0) / 360.0).floor() * 360.0;

    Some((rlat, rlon))
}

/// Decode surface CPR position from even/odd frame pair with reference.
/// Matches decodeCPRsurface() in cpr.c.
pub fn decode_cpr_surface(
    ref_lat: f64, ref_lon: f64,
    even_cprlat: i32, even_cprlon: i32,
    odd_cprlat: i32, odd_cprlon: i32,
    fflag: i32,
) -> Option<(f64, f64)> {
    const CPR_RES: f64 = 131072.0;

    let dlat_even = 90.0 / 60.0;    // 1.5°
    let dlat_odd  = 90.0 / 59.0;    // ~1.525°

    let lat0 = even_cprlat as f64;
    let lat1 = odd_cprlat  as f64;
    let lon0 = even_cprlon as f64;
    let lon1 = odd_cprlon  as f64;

    // Compute the Latitude Index "j"
    let j = ((59.0 * lat0 - 60.0 * lat1) / CPR_RES + 0.5).floor() as i32;

    let mut rlat0 = dlat_even * (cpr_mod_int(j, 60) as f64 + lat0 / CPR_RES);
    let mut rlat1 = dlat_odd  * (cpr_mod_int(j, 59) as f64 + lat1 / CPR_RES);

    // Pick quadrant closest to reference latitude (surface-specific logic)
    if rlat0 == 0.0 {
        if ref_lat < -45.0 {
            rlat0 = -90.0;
        } else if ref_lat > 45.0 {
            rlat0 = 90.0;
        }
    } else if (rlat0 - ref_lat) > 45.0 {
        rlat0 -= 90.0;
    }

    if rlat1 == 0.0 {
        if ref_lat < -45.0 {
            rlat1 = -90.0;
        } else if ref_lat > 45.0 {
            rlat1 = 90.0;
        }
    } else if (rlat1 - ref_lat) > 45.0 {
        rlat1 -= 90.0;
    }

    // Check latitude range
    if rlat0 < -90.0 || rlat0 > 90.0 || rlat1 < -90.0 || rlat1 > 90.0 {
        return None;
    }

    // Check that both positions are in the same latitude zone
    if cpr_nl(rlat0) != cpr_nl(rlat1) {
        return None;
    }

    // Compute longitude
    let (rlat, mut rlon) = if fflag != 0 {
        // Use odd packet
        let ni = cpr_n_func(rlat1, 1);
        let m = ((lon0 * (cpr_nl(rlat1) - 1) as f64 - lon1 * cpr_nl(rlat1) as f64)
            / CPR_RES + 0.5).floor() as i32;
        let mut rlon = cpr_dlon_function(rlat1, 1, true)
            * (cpr_mod_int(m, ni) as f64 + lon1 / CPR_RES);
        // Pick longitude quadrant closest to reference
        rlon += ((ref_lon - rlon + 45.0) / 90.0).floor() * 90.0;
        (rlat1, rlon)
    } else {
        // Use even packet
        let ni = cpr_n_func(rlat0, 0);
        let m = ((lon0 * (cpr_nl(rlat0) - 1) as f64 - lon1 * cpr_nl(rlat0) as f64)
            / CPR_RES + 0.5).floor() as i32;
        let mut rlon = cpr_dlon_function(rlat0, 0, true)
            * (cpr_mod_int(m, ni) as f64 + lon0 / CPR_RES);
        // Pick longitude quadrant closest to reference
        rlon += ((ref_lon - rlon + 45.0) / 90.0).floor() * 90.0;
        (rlat0, rlon)
    };

    // Renormalize longitude to -180 .. +180
    rlon -= ((rlon + 180.0) / 360.0).floor() * 360.0;

    Some((rlat, rlon))
}

/// Decode CPR position relative to a reference position (single frame).
/// Matches decodeCPRrelative() in cpr.c.
pub fn decode_cpr_relative(
    ref_lat: f64, ref_lon: f64,
    cprlat: i32, cprlon: i32,
    fflag: i32,
    surface: bool,
) -> Option<(f64, f64)> {
    const CPR_RES: f64 = 131072.0;

    let fractional_lat = cprlat as f64 / CPR_RES;
    let fractional_lon = cprlon as f64 / CPR_RES;

    let dlat = if surface { 90.0 } else { 360.0 } / if fflag != 0 { 59.0 } else { 60.0 };

    // Compute the Latitude Index "j"
    let j = (ref_lat / dlat).floor() as i32
        + (0.5 + cpr_mod_double(ref_lat, dlat) / dlat - fractional_lat).floor() as i32;

    let mut rlat = dlat * (j as f64 + fractional_lat);

    // Normalize latitude
    if rlat >= 270.0 { rlat -= 360.0; }

    // Check latitude range
    if rlat < -90.0 || rlat > 90.0 {
        return None;
    }

    // Check that answer is reasonable — no more than 1/2 cell away
    if (rlat - ref_lat).abs() > (dlat / 2.0) {
        return None;
    }

    // Compute the Longitude Index "m"
    let dlon = cpr_dlon_function(rlat, fflag, surface);
    let m = (ref_lon / dlon).floor() as i32
        + (0.5 + cpr_mod_double(ref_lon, dlon) / dlon - fractional_lon).floor() as i32;

    let mut rlon = dlon * (m as f64 + fractional_lon);

    // Normalize longitude
    if rlon > 180.0 { rlon -= 360.0; }

    // Check that answer is reasonable
    if (rlon - ref_lon).abs() > (dlon / 2.0) {
        return None;
    }

    Some((rlat, rlon))
}
