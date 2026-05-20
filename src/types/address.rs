#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DataSource {
    Invalid = 0,
    Indirect = 1,
    ModeAc = 2,
    Sbs = 3,
    Mlat = 4,
    ModeS = 5,
    Jaero = 6,
    ModeSChecked = 7,
    Tisb = 8,
    Adsr = 9,
    Nt = 10,
    Adsb = 11,
    Priority = 12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AddrType {
    AdsbIcao = 0,
    AdsbIcaoNt = 1,
    AdsrIcao = 2,
    TisbIcao = 3,
    Jaero = 4,
    Mlat = 5,
    Other = 6,
    ModeS = 7,
    AdsbOther = 8,
    AdsrOther = 9,
    TisbTrackfile = 10,
    TisbOther = 11,
    ModeA = 12,
    Unknown = 13,
}

pub const NON_ICAO_ADDRESS: u32 = 1 << 24;
pub const BADDR: u32 = 0xff123456;
