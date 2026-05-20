pub const GLOBE_INDEX_GRID: f64 = 3.0;
pub const GLOBE_LAT_MULT: f64 = 360.0 / GLOBE_INDEX_GRID + 1.0;
pub const GLOBE_MIN_INDEX: i32 = 1000;

pub fn globe_index(lat: f64, lon: f64) -> i32 {
    let lat_idx = ((lat + 90.0) / GLOBE_INDEX_GRID) as i32;
    let lon_idx = ((lon + 180.0) / GLOBE_INDEX_GRID) as i32;
    GLOBE_MIN_INDEX + lat_idx * GLOBE_LAT_MULT as i32 + lon_idx
}

pub struct StatePoint {
    pub timestamp: i64,
    pub lat: i32,
    pub lon: i32,
    pub baro_alt: i16,
    pub gs: u16,
    pub track: u16,
    pub flags: u16,
}
