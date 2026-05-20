#[repr(C, packed)]
pub struct HeatEntry {
    pub hex: i32,
    pub lat: i32,
    pub lon: i32,
    pub alt: i16,
    pub gs: i16,
}
