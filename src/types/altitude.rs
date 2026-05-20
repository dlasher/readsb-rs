#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeUnit {
    Feet,
    Meters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeSource {
    Baro,
    Geom,
}

pub const INVALID_ALTITUDE: i32 = -9999;
