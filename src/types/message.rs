use crate::types::address::{DataSource, AddrType};
use crate::types::altitude::{AltitudeUnit, INVALID_ALTITUDE};
use crate::types::status::{CprType, AirGround, Emergency};
use crate::types::accuracy::{MessageAccuracy, OpStatus, NavState};
use super::comm_b::CommBFormat;

/// Decoded Mode S message.
/// Mirrors `struct modesMessage` from readsb.h:990-1247.
#[derive(Debug, Clone)]
pub struct ModesMessage {
    // Raw data
    pub msg: [u8; 14],
    pub verbatim: [u8; 14],

    // Timing and source
    pub timestamp: i64,
    pub sys_timestamp: i64,
    pub receiver_id: u64,
    pub source: DataSource,
    pub addrtype: AddrType,
    pub remote: bool,

    // Message identification
    pub msgtype: i32,
    pub msgbits: i32,
    pub crc: u32,
    pub corrected_bits: i32,
    pub crc_ok: bool,
    pub corrected: bool,
    pub addr: u32,

    // Extracted raw fields
    pub ca: u32,
    pub cf: u32,
    pub aa: u32,
    pub me: [u8; 7],
    pub mb: [u8; 7],
    pub md: [u8; 10],
    pub mv: [u8; 7],
    pub metype: u32,
    pub mesub: u32,
    pub commb_format: CommBFormat,

    // Decoded data flags
    pub baro_alt_valid: bool,
    pub geom_alt_valid: bool,
    pub track_valid: bool,
    pub gs_valid: bool,
    pub ias_valid: bool,
    pub tas_valid: bool,
    pub mach_valid: bool,
    pub baro_rate_valid: bool,
    pub geom_rate_valid: bool,
    pub squawk_valid: bool,
    pub callsign_valid: bool,
    pub cpr_valid: bool,
    pub category_valid: bool,

    // Decoded values
    pub baro_alt: i32,
    pub baro_alt_unit: AltitudeUnit,
    pub geom_alt: i32,
    pub geom_alt_unit: AltitudeUnit,
    pub geom_delta: i32,
    pub gs: f32,
    pub track: f32,
    pub ias: u32,
    pub tas: u32,
    pub mach: f64,
    pub baro_rate: i32,
    pub geom_rate: i32,
    pub squawk_hex: u32,
    pub callsign: [u8; 16],
    pub category: u8,
    pub emergency: Emergency,

    // CPR
    pub cpr_type: CprType,
    pub cpr_lat: u32,
    pub cpr_lon: u32,
    pub cpr_odd: bool,
    pub cpr_decoded: bool,
    pub cpr_relative: bool,
    pub decoded_lat: f64,
    pub decoded_lon: f64,

    // Air/ground
    pub airground: AirGround,

    // Accuracy fields
    pub accuracy: MessageAccuracy,
    pub op_status: Option<OpStatus>,

    // Navigation
    pub nav: NavState,

    // Signal
    pub signal_level: f64,
}

impl Default for ModesMessage {
    fn default() -> Self {
        ModesMessage {
            msg: [0; 14], verbatim: [0; 14],
            timestamp: 0, sys_timestamp: 0, receiver_id: 0,
            source: DataSource::Invalid, addrtype: AddrType::Unknown, remote: false,
            msgtype: 0, msgbits: 0, crc: 0, corrected_bits: 0,
            crc_ok: false, corrected: false, addr: 0,
            ca: 0, cf: 0, aa: 0,
            me: [0; 7], mb: [0; 7], md: [0; 10], mv: [0; 7],
            metype: 0, mesub: 0, commb_format: CommBFormat::default(),
            baro_alt_valid: false, geom_alt_valid: false,
            track_valid: false, gs_valid: false, ias_valid: false,
            tas_valid: false, mach_valid: false, baro_rate_valid: false,
            geom_rate_valid: false, squawk_valid: false, callsign_valid: false,
            cpr_valid: false, category_valid: false,
            baro_alt: INVALID_ALTITUDE, baro_alt_unit: AltitudeUnit::Feet,
            geom_alt: INVALID_ALTITUDE, geom_alt_unit: AltitudeUnit::Feet,
            geom_delta: 0, gs: 0.0, track: 0.0, ias: 0, tas: 0, mach: 0.0,
            baro_rate: 0, geom_rate: 0, squawk_hex: 0,
            callsign: [0; 16], category: 0, emergency: Emergency::None,
            cpr_type: CprType::Invalid, cpr_lat: 0, cpr_lon: 0,
            cpr_odd: false, cpr_decoded: false, cpr_relative: false,
            decoded_lat: 0.0, decoded_lon: 0.0,
            airground: AirGround::Invalid,
            accuracy: MessageAccuracy::default(),
            op_status: None,
            nav: NavState::default(),
            signal_level: 0.0,
        }
    }
}
