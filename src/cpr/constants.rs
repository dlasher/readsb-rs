/// Number of latitude zones for airborne CPR
pub const CPR_NZ: usize = 15;

/// CPR encoding resolution (2^17 for airborne, 2^14 for surface)
pub const CPR_AIRBORNE_RES: f64 = 131072.0; // 2^17
pub const CPR_SURFACE_RES: f64 = 16384.0;   // 2^14

/// NL(lat) table — number of longitude zones at each latitude band.
/// Indexed by floor(lat * 59 / 360) at the equator.
/// From cpr.c (cprNL function table).
pub const NL_TABLE: [i32; 59] = [
    59, 59, 59, 59, 58, 58, 58, 57, 57, 57, 57,
    56, 56, 56, 56, 55, 55, 55, 55, 54, 54, 54,
    54, 53, 53, 53, 53, 52, 52, 52, 52, 51, 51,
    51, 51, 50, 50, 50, 50, 49, 49, 49, 49, 48,
    48, 48, 48, 47, 47, 47, 47, 46, 46, 46, 46,
    45, 45, 45, 45,
];
