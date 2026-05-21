use crate::types::*;
use crate::crc::{modes_checksum, CrcFixEngine};

pub struct ParseResult {
    pub message: ModesMessage,
    pub crc_ok: bool,
    pub corrected: bool,
}

#[allow(clippy::field_reassign_with_default)]
pub fn parse_modes_message(msg: &[u8], msgbits: usize, crc_engine: &CrcFixEngine, signal_level: f64) -> Option<ParseResult> {
    if msgbits != 56 && msgbits != 112 { return None; }
    let mut mm = ModesMessage::default();
    mm.signal_level = signal_level;
    mm.msgbits = msgbits as i32;
    let nbytes = msgbits / 8;
    let mut padded = [0u8; 14];
    padded[..nbytes].copy_from_slice(&msg[..nbytes]);
    mm.msg = padded; mm.verbatim = padded;
    mm.msgtype = ((padded[0] & 0xF8) >> 3) as i32;
    let crc = modes_checksum(&padded, msgbits); mm.crc = crc;
    if crc != 0 {
        if let Some(info) = crc_engine.diagnose(crc) {
            CrcFixEngine::fix(&mut mm.msg, info);
            mm.corrected_bits = info.errors as i32;
            mm.crc = modes_checksum(&mm.msg, msgbits); mm.corrected = true;
        }
    }
    mm.crc_ok = mm.crc == 0;
    extract_common_fields(&mut mm);
    match mm.msgtype {
        0 | 4 | 16 => decode_df0_4_16(&mut mm),
        5 | 21 => decode_df5_21(&mut mm), 11 => decode_df11(&mut mm),
        17 | 18 => decode_df17_18(&mut mm), 19 => decode_df19(&mut mm),
        20 => decode_df20(&mut mm), 24 => decode_df24(&mut mm), 31 => decode_df31(&mut mm),
        _ => {}
    }
    let crc_ok = mm.crc_ok;
    let corrected = mm.corrected;
    Some(ParseResult { message: mm, crc_ok, corrected })
}

fn extract_common_fields(mm: &mut ModesMessage) {
    mm.cf = ((mm.msg[0] & 0xE0) >> 5) as u32;
    mm.ca = (mm.msg[0] & 0x07) as u32;
    if mm.msgtype == 11 || mm.msgtype == 17 || mm.msgtype == 18 {
        mm.aa = ((mm.msg[1] as u32) << 16) | ((mm.msg[2] as u32) << 8) | (mm.msg[3] as u32);
        mm.addr = mm.aa;
    }
}

fn decode_df0_4_16(mm: &mut ModesMessage) {
    let ac = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    if ac != 0 { mm.baro_alt = decode_altitude(ac); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet; }
    if mm.msgtype == 4 || mm.msgtype == 16 {
        mm.addr = ((mm.msg[4] as u32 & 0x0F) << 20) | ((mm.msg[5] as u32) << 12) | ((mm.msg[6] as u32) << 4) | ((mm.msg[7] as u32 & 0xF0) >> 4);
    }
}

fn decode_df5_21(mm: &mut ModesMessage) {
    let id = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    mm.squawk_hex = ((id & 0x1F00) << 1) | ((id & 0x003F) << 2) | ((id & 0x00C0) >> 6);
    mm.squawk_valid = true;
}

fn decode_df11(_mm: &mut ModesMessage) {}

fn decode_df17_18(mm: &mut ModesMessage) {
    mm.me.copy_from_slice(&mm.msg[5..12]);
    mm.metype = ((mm.me[0] & 0xF8) >> 3) as u32;
    mm.mesub = (mm.me[0] & 0x07) as u32;
    match mm.metype {
        1..=4 => decode_surface_position(mm),
        5..=8 => decode_airborne_position(mm),
        9..=19 => decode_airborne_velocity(mm),
        20 => decode_target_state(mm),
        21 | 28 | 31 => decode_aircraft_status(mm),
        _ => {}
    }
}

fn decode_df19(_mm: &mut ModesMessage) {}
fn decode_df20(mm: &mut ModesMessage) {
    let ac = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    if ac != 0 { mm.baro_alt = decode_altitude(ac); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet; }
    mm.mb.copy_from_slice(&mm.msg[5..12]);
    mm.commb_format = crate::modes::comm_b::decode_comm_b(&mm.mb);
}
fn decode_df24(_mm: &mut ModesMessage) {}
fn decode_df31(mm: &mut ModesMessage) { mm.mv.copy_from_slice(&mm.msg[5..12]); }

pub fn decode_altitude(ac: u32) -> i32 {
    if ac & 0x0040 != 0 {
        let n = ((ac & 0x003F) << 1) | ((ac & 0x0FC0) >> 6);
        if n == 0 { return INVALID_ALTITUDE; }
        ((n as i32) - 10) * 100
    } else { INVALID_ALTITUDE }
}

fn decode_surface_position(mm: &mut ModesMessage) {
    mm.cpr_type = CprType::Surface;
    mm.cpr_lat = ((mm.me[1] as u32 & 0x03) << 15) | ((mm.me[2] as u32) << 7) | ((mm.me[3] as u32 & 0xFE) >> 1);
    mm.cpr_lon = ((mm.me[3] as u32 & 0x01) << 16) | ((mm.me[4] as u32) << 8) | (mm.me[5] as u32);
    mm.cpr_odd = (mm.me[0] & 0x04) != 0; mm.cpr_valid = true; mm.cpr_decoded = true;
    let mvm = mm.me[6] >> 2;
    if mvm > 0 && mvm < 125 { mm.gs = MOVEMENT_TABLE[mvm as usize]; mm.gs_valid = true; }
    mm.airground = if mm.me[6] & 0x01 != 0 { AirGround::Ground } else { AirGround::Airborne };
}

fn decode_airborne_position(mm: &mut ModesMessage) {
    mm.cpr_type = CprType::Airborne;
    mm.cpr_lat = ((mm.me[1] as u32 & 0x03) << 15) | ((mm.me[2] as u32) << 7) | ((mm.me[3] as u32 & 0xFE) >> 1);
    mm.cpr_lon = ((mm.me[3] as u32 & 0x01) << 16) | ((mm.me[4] as u32) << 8) | (mm.me[5] as u32);
    mm.cpr_odd = (mm.me[0] & 0x04) != 0; mm.cpr_valid = true; mm.cpr_decoded = true;
    if (mm.me[5] & 0x04) != 0 {
        let raw = (((mm.me[5] as u32 & 0x10) << 4) | ((mm.me[5] as u32 & 0x03) << 8) | (mm.me[6] as u32 & 0xFC)) >> 2;
        mm.geom_alt = (raw as i32 - 1000) * 25; mm.geom_alt_valid = true; mm.geom_alt_unit = AltitudeUnit::Feet;
    } else {
        let raw = ((mm.me[5] as u32 & 0x10) << 1) | ((mm.me[5] as u32 & 0x03) << 8) | ((mm.me[6] as u32 & 0xFC) >> 2);
        mm.baro_alt = decode_altitude(raw); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet;
    }
}

fn decode_airborne_velocity(mm: &mut ModesMessage) {
    if mm.mesub == 1 || mm.mesub == 2 {
        let _ew_sign = (mm.me[2] & 0x80) != 0;
        let ew_vel = (((mm.me[2] as u32 & 0x7F) << 3) | ((mm.me[3] as u32 & 0xE0) >> 5)).saturating_sub(1);
        let _ns_sign = (mm.me[3] & 0x10) != 0;
        let ns_vel = (((mm.me[3] as u32 & 0x0F) << 6) | ((mm.me[4] as u32 & 0xFC) >> 2)).saturating_sub(1);
        if ew_vel > 0 && ns_vel > 0 {
            let mult = if mm.mesub == 1 { 1.0 } else { 4.0 };
            let ew = ew_vel as f32 * mult; let ns = ns_vel as f32 * mult;
            mm.gs = (ew * ew + ns * ns).sqrt(); mm.gs_valid = true;
        }
        let vr_sign = (mm.me[4] & 0x01) != 0;
        let vr = ((mm.me[5] as u32 & 0x7F) << 1) | ((mm.me[6] as u32 & 0x80) >> 7);
        if vr > 0 { mm.geom_rate = (vr as i32 - 1) * 64; if vr_sign { mm.geom_rate = -mm.geom_rate; } mm.geom_rate_valid = true; }
    }
}

fn decode_target_state(mm: &mut ModesMessage) {
    if (mm.me[1] & 0x80) != 0 {
        let alt = ((mm.me[1] as u32 & 0x7F) << 4) | ((mm.me[2] as u32 & 0xF0) >> 4);
        mm.nav.mcp_altitude = alt * 16; mm.nav.mcp_altitude_valid = true;
    }
}

fn decode_aircraft_status(mm: &mut ModesMessage) {
    if mm.metype == 28 && mm.mesub == 1 {
        match mm.me[1] { 1 => mm.emergency = Emergency::General, 2 => mm.emergency = Emergency::Lifeguard, 3 => mm.emergency = Emergency::Minfuel, 4 => mm.emergency = Emergency::Nordo, 5 => mm.emergency = Emergency::Unlawful, 6 => mm.emergency = Emergency::Downed, _ => {} }
    }
}

#[rustfmt::skip]
const MOVEMENT_TABLE: [f32; 125] = [
    0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0,
    10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0,
    20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0,
    30.0, 31.0, 32.0, 33.0, 34.0, 35.0, 36.0, 37.0, 38.0, 39.0,
    40.0, 41.0, 42.0, 43.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0,
    50.0, 51.0, 52.0, 53.0, 54.0, 55.0, 56.0, 57.0, 58.0, 59.0,
    60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0,
    70.0, 71.0, 72.0, 73.0, 74.0, 75.0, 76.0, 77.0, 78.0, 79.0,
    80.0, 81.0, 82.0, 83.0, 84.0, 85.0, 86.0, 87.0, 88.0, 89.0,
    90.0, 91.0, 92.0, 93.0, 94.0, 95.0, 96.0, 97.0, 98.0, 99.0,
    100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0,
    110.0, 111.0, 112.0, 113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0,
    120.0, 121.0, 122.0, 123.0, 124.0,
];
