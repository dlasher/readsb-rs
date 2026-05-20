#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AirGround {
    Invalid = 0,
    Ground = 1,
    Airborne = 2,
    Uncertain = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Emergency {
    None = 0,
    General = 1,
    Lifeguard = 2,
    Minfuel = 3,
    Nordo = 4,
    Unlawful = 5,
    Downed = 6,
    Reserved = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SilType {
    Invalid,
    Unknown,
    PerSample,
    PerHour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CprType {
    Invalid,
    Surface,
    Airborne,
    Coarse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeadingType {
    Invalid,
    GroundTrack,
    True,
    Magnetic,
    MagneticOrTrue,
    TrackOrHeading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NavAltitudeSource {
    Invalid,
    Unknown,
    Aircraft,
    Mcp,
    Fms,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NavModes: u8 {
        const AUTOPILOT = 1;
        const VNAV = 2;
        const ALT_HOLD = 4;
        const APPROACH = 8;
        const LNAV = 16;
        const TCAS = 32;
    }
}
