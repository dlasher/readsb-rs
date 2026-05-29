use crate::types::*;
use crate::crc::{modes_checksum, CrcFixEngine};

pub struct ParseResult {
    pub message: ModesMessage,
    pub crc_ok: bool,
    pub corrected: bool,
}

#[allow(clippy::field_reassign_with_default)]
pub fn parse_modes_message(msg: &[u8], msgbits: usize, crc_engine: &CrcFixEngine, signal_level: f64) -> Option<ParseResult> {
    if msgbits == 16 {
        // Mode A/C frame: 2-byte raw Mode A code, no ICAO, no CRC
        let mut mm = ModesMessage::default();
        mm.signal_level = signal_level;
        mm.msgbits = msgbits as i32;
        let modea = ((msg[0] as u32) << 4) | ((msg[1] as u32) >> 4);
        mm.msgtype = ((msg[0] & 0xF8) >> 3) as i32;
        mm.msg[..2].copy_from_slice(&msg[..2]);
        mm.verbatim[..2].copy_from_slice(&msg[..2]);
        decode_mode_ac(&mut mm, modea);
        mm.crc_ok = true;
        return Some(ParseResult { message: mm, crc_ok: true, corrected: false });
    }
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

    // Address/Parity msgtypes: the CRC syndrome IS the sender's ICAO address.
    let is_address_parity = matches!(mm.msgtype, 0 | 4 | 5);
    if is_address_parity {
        mm.addr = crc;
        mm.crc_ok = true;
    } else if crc != 0 {
        if let Some(info) = crc_engine.diagnose(crc) {
            CrcFixEngine::fix(&mut mm.msg, info);
            mm.corrected_bits = info.errors as i32;
            mm.crc = modes_checksum(&mm.msg, msgbits); mm.corrected = true;
        }
        mm.crc_ok = mm.crc == 0;
    } else {
        mm.crc_ok = true;
    }
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

fn decode_mode_ac(mm: &mut ModesMessage, modea: u32) {
    // Mode A code: 12 bits, only A/B/C/D pulses (mask 0x7777 strips SPI/IDENT at bit 7)
    mm.squawk_hex = modea & 0x7777;
    mm.squawk_valid = true;
    // Fudge a non-ICAO address so the tracker can store this
    mm.addr = (modea & 0x0000_007F) | 0x0100_0000;
    mm.crc_ok = true;
}

fn decode_df11(_mm: &mut ModesMessage) {}

fn decode_df17_18(mm: &mut ModesMessage) {
    mm.me.copy_from_slice(&mm.msg[4..11]);
    mm.metype = ((mm.me[0] & 0xF8) >> 3) as u32;
    mm.mesub = (mm.me[0] & 0x07) as u32;
    match mm.metype {
        1..=4 => decode_aircraft_identification(mm),
        5..=8 => decode_surface_position(mm),
        9..=18 => decode_airborne_position(mm),
        19 => decode_airborne_velocity(mm),
        20 => decode_target_state(mm),
        21 | 28 | 31 => decode_aircraft_status(mm),
        _ => {}
    }
}

fn decode_df19(_mm: &mut ModesMessage) {}
fn decode_df20(mm: &mut ModesMessage) {
    let ac = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    if ac != 0 { mm.baro_alt = decode_altitude(ac); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet; }
    mm.mb.copy_from_slice(&mm.msg[4..11]);
    mm.commb_format = crate::modes::comm_b::decode_comm_b(&mm.mb);
    if mm.commb_format == CommBFormat::AircraftIdent {
        let cs = crate::modes::comm_b::commb_callsign(&mm.mb);
        let bytes = cs.as_bytes();
        let len = bytes.len().min(16);
        mm.callsign[..len].copy_from_slice(&bytes[..len]);
        mm.callsign_valid = true;
    }
}
fn decode_df24(_mm: &mut ModesMessage) {}
fn decode_df31(mm: &mut ModesMessage) { mm.mv.copy_from_slice(&mm.msg[4..11]); }

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

fn decode_aircraft_identification(mm: &mut ModesMessage) {
    use crate::modes::ais::ais_6bit_to_ascii;
    let bits: u64 = mm.me[1..7].iter().fold(0u64, |acc, &b| (acc << 8) | b as u64);
    let mut chars = [0u8; 8];
    let mut pos = 0;
    for i in (0..48).step_by(6).rev() {
        let ch = ((bits >> i) & 0x3F) as u8;
        let c = ais_6bit_to_ascii(ch);
        if c == '@' { break; }
        chars[pos] = c as u8;
        pos += 1;
    }
    let cs = std::str::from_utf8(&chars[..pos]).unwrap_or("").trim().to_string();
    if !cs.is_empty() {
        let bytes = cs.as_bytes();
        let len = bytes.len().min(16);
        mm.callsign[..len].copy_from_slice(&bytes[..len]);
        mm.callsign_valid = true;
    }
}

fn decode_airborne_position(mm: &mut ModesMessage) {
    mm.cpr_type = CprType::Airborne;
    // Bits 23-39: CPR latitude (me[2] bits 1-0, me[3] bits 7-0, me[4] bits 7-1)
    mm.cpr_lat = ((mm.me[2] as u32 & 0x03) << 15) | ((mm.me[3] as u32) << 7) | ((mm.me[4] as u32 & 0xFE) >> 1);
    // Bits 40-56: CPR longitude (me[4] bit 0, me[5] bits 7-0, me[6] bits 7-0)
    mm.cpr_lon = ((mm.me[4] as u32 & 0x01) << 16) | ((mm.me[5] as u32) << 8) | (mm.me[6] as u32);
    // Bit 22 = F flag (me[2] bit 2)
    mm.cpr_odd = (mm.me[2] & 0x04) != 0; mm.cpr_valid = true; mm.cpr_decoded = true;
    // Bits 9-20 = 12-bit altitude field (me[1] bits 7-0, me[2] bits 7-4)
    let alt_raw = ((mm.me[1] as u16) << 4) | ((mm.me[2] as u16) >> 4);
    if alt_raw > 0 && alt_raw & 0x10 != 0 {
        // Q-bit set: 25ft encoding
        let n = (((alt_raw & 0x0FE0) >> 1) | (alt_raw & 0x000F)) as i32;
        let alt = n * 25 - 1000;
        if mm.metype >= 20 && mm.metype <= 22 {
            mm.geom_alt = alt; mm.geom_alt_valid = true; mm.geom_alt_unit = AltitudeUnit::Feet;
        } else {
            mm.baro_alt = alt; mm.baro_alt_valid = true; mm.baro_alt_unit = AltitudeUnit::Feet;
        }
    }
}

fn decode_airborne_velocity(mm: &mut ModesMessage) {
    if mm.mesub == 1 || mm.mesub == 2 {
        // EW velocity: C bits 15-24 (MSB-first) = me[1] b1,b0 + me[2] b7..b0
        let ew_raw = ((mm.me[1] as u32 & 0x03) << 8) | (mm.me[2] as u32);
        // NS velocity: C bits 26-35 = me[3] b6..b0 + me[4] b7..b5
        let ns_raw = ((mm.me[3] as u32 & 0x7F) << 3) | ((mm.me[4] as u32) >> 5);
        // EW direction: C bit 14 = me[1] b2 (1=East, 0=West)
        let ew_sign = (mm.me[1] & 0x04) != 0;
        // NS direction: C bit 25 = me[3] b7 (1=South, 0=North)
        let ns_sign = (mm.me[3] & 0x80) != 0;
        if ew_raw > 0 && ns_raw > 0 {
            let ew_vel = (ew_raw - 1) as f32;
            let ns_vel = (ns_raw - 1) as f32;
            let mult = if mm.mesub == 1 { 1.0 } else { 4.0 };
            let ew = if ew_sign { -(ew_vel * mult) } else { ew_vel * mult };
            let ns = if ns_sign { -(ns_vel * mult) } else { ns_vel * mult };
            mm.gs = (ew * ew + ns * ns).sqrt(); mm.gs_valid = true;
            mm.track = ew.atan2(ns).to_degrees();
            if mm.track < 0.0 { mm.track += 360.0; }
            mm.track_valid = true;
        }
        // Vertical rate: C bits 37-46 = me[4] b6..b3 + me[5] b7..b2
        let vr_raw = (((mm.me[4] as u32 >> 3) & 0x0F) << 6) | ((mm.me[5] as u32 >> 2) & 0x3F);
        // VR direction: C bit 36 = me[4] b7 (1=down, 0=up)
        let vr_sign = (mm.me[4] & 0x80) != 0;
        if vr_raw > 0 {
            let vr = vr_raw - 1;
            mm.geom_rate = if vr_sign { -(vr as i32 * 64) } else { vr as i32 * 64 };
            mm.geom_rate_valid = true;
        }
    }
}

fn decode_target_state(mm: &mut ModesMessage) {
    if (mm.me[1] & 0x80) != 0 {
        let alt = ((mm.me[1] as u32 & 0x7F) << 4) | ((mm.me[2] as u32 & 0xF0) >> 4);
        mm.nav.mcp_altitude = alt * 16; mm.nav.mcp_altitude_valid = true;
    }
}

fn decode_id13(id13: u32) -> u32 {
    let mut hex = 0u32;
    if id13 & 0x1000 != 0 { hex |= 0x0010; }
    if id13 & 0x0800 != 0 { hex |= 0x1000; }
    if id13 & 0x0400 != 0 { hex |= 0x0020; }
    if id13 & 0x0200 != 0 { hex |= 0x2000; }
    if id13 & 0x0100 != 0 { hex |= 0x0040; }
    if id13 & 0x0080 != 0 { hex |= 0x4000; }
    if id13 & 0x0020 != 0 { hex |= 0x0100; }
    if id13 & 0x0010 != 0 { hex |= 0x0001; }
    if id13 & 0x0008 != 0 { hex |= 0x0200; }
    if id13 & 0x0004 != 0 { hex |= 0x0002; }
    if id13 & 0x0002 != 0 { hex |= 0x0400; }
    if id13 & 0x0001 != 0 { hex |= 0x0004; }
    hex
}

fn decode_aircraft_status(mm: &mut ModesMessage) {
    if mm.metype == 28 && mm.mesub == 7 {
        let id13 = ((mm.me[1] as u32) << 5) | ((mm.me[2] as u32) >> 3);
        if id13 != 0 {
            mm.squawk_hex = decode_id13(id13);
            mm.squawk_valid = true;
        }
    }
    if (mm.metype == 28 || mm.metype == 31) && mm.mesub == 1 {
        match mm.me[1] >> 5 {
            1 => mm.emergency = Emergency::General,
            2 => mm.emergency = Emergency::Lifeguard,
            3 => mm.emergency = Emergency::Minfuel,
            4 => mm.emergency = Emergency::Nordo,
            5 => mm.emergency = Emergency::Unlawful,
            6 => mm.emergency = Emergency::Downed,
            _ => {}
        }
        let id13 = (((mm.me[1] as u32) & 0x1F) << 8) | (mm.me[2] as u32);
        if id13 != 0 {
            mm.squawk_hex = decode_id13(id13);
            mm.squawk_valid = true;
        }
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
