use std::sync::LazyLock;
use crate::crc::{modes_checksum, CrcFixEngine};

#[derive(Clone, Debug)]
pub struct Message {
    pub bytes: Vec<u8>,
    pub signal: f64,
}

#[derive(Clone, Debug, Default)]
pub struct MagBufStats {
    pub loud_events: u32,
    pub noise_low_samples: u32,
    pub noise_high_samples: u32,
}

#[derive(Clone, Debug)]
pub struct DemodResult {
    pub messages: Vec<Message>,
    pub stats: MagBufStats,
}

#[derive(Clone, Debug)]
pub struct DemodConfig {
    pub preamble_threshold: u32,
    pub fix_df: bool,
    pub auto_gain: bool,
}

impl Default for DemodConfig {
    fn default() -> Self {
        DemodConfig {
            preamble_threshold: 58,
            fix_df: false,
            auto_gain: false,
        }
    }
}

impl DemodConfig {
    pub fn new() -> Self {
        DemodConfig::default()
    }
}

static CRC_ENGINE_112: LazyLock<CrcFixEngine> = LazyLock::new(|| CrcFixEngine::new(112));
static CRC_ENGINE_56: LazyLock<CrcFixEngine> = LazyLock::new(|| CrcFixEngine::new(56));

#[inline(always)]
pub fn slice_phase0(m: &[u16]) -> i32 { 18 * m[0] as i32 - 15 * m[1] as i32 - 3 * m[2] as i32 }
#[inline(always)]
pub fn slice_phase1(m: &[u16]) -> i32 { 14 * m[0] as i32 - 5 * m[1] as i32 - 9 * m[2] as i32 }
#[inline(always)]
pub fn slice_phase2(m: &[u16]) -> i32 { 16 * m[0] as i32 + 5 * m[1] as i32 - 20 * m[2] as i32 }
#[inline(always)]
pub fn slice_phase3(m: &[u16]) -> i32 { 7 * m[0] as i32 + 11 * m[1] as i32 - 18 * m[2] as i32 }
#[inline(always)]
pub fn slice_phase4(m: &[u16]) -> i32 { 4 * m[0] as i32 + 15 * m[1] as i32 - 20 * m[2] as i32 + m[3] as i32 }

const SLICE_FUNCS: [fn(&[u16]) -> i32; 5] = [
    slice_phase0, slice_phase1, slice_phase2, slice_phase3, slice_phase4,
];

const SLICE_OFFSETS: [[usize; 8]; 5] = [
    [0, 2, 4, 7, 9, 12, 14, 16],
    [0, 2, 5, 7, 9, 12, 14, 17],
    [0, 2, 5, 7, 10, 12, 14, 17],
    [0, 3, 5, 7, 10, 12, 15, 17],
    [0, 3, 5, 8, 10, 12, 15, 17],
];

const SLICE_ADVANCE: [usize; 5] = [19, 19, 19, 19, 20];
const SLICE_NEXT_PHASE: [usize; 5] = [1, 2, 3, 4, 0];

const SLICE_FUNC_MAP: [[usize; 8]; 5] = [
    [0, 2, 4, 1, 3, 0, 2, 4],
    [1, 3, 0, 2, 4, 1, 3, 0],
    [2, 4, 1, 3, 0, 2, 4, 1],
    [3, 0, 2, 4, 1, 3, 0, 2],
    [4, 1, 3, 0, 2, 4, 1, 3],
];

pub fn slice_byte(mag: &[u16], pos: &mut usize, phase: &mut usize) -> u8 {
    let p = *pos;
    let pidx = *phase;
    let offsets = &SLICE_OFFSETS[pidx];
    let funcmap = &SLICE_FUNC_MAP[pidx];
    let mut byte = 0u8;
    for bit in 0..8 {
        let func_idx = funcmap[bit];
        let result = SLICE_FUNCS[func_idx](&mag[p + offsets[bit]..]);
        if result > 0 {
            byte |= 1 << (7 - bit);
        }
    }
    *pos += SLICE_ADVANCE[pidx];
    *phase = SLICE_NEXT_PHASE[pidx];
    byte
}

const MODES_LONG_MSG_SAMPLES: usize = 269;
const MODES_LONG_MSG_BYTES: usize = 14;
const MODES_SHORT_MSG_BYTES: usize = 7;

const VALID_DF_SHORT: u32 = (1 << 0) | (1 << 4) | (1 << 5) | (1 << 11);
const VALID_DF_LONG: u32 = (1 << 16) | (1 << 17) | (1 << 18) | (1 << 20) | (1 << 21);

fn modes_message_len_by_type(df: u32) -> usize {
    if (VALID_DF_LONG & (1 << df)) != 0 {
        MODES_LONG_MSG_BYTES
    } else if (VALID_DF_SHORT & (1 << df)) != 0 {
        MODES_SHORT_MSG_BYTES
    } else {
        0
    }
}

pub fn score_phase(try_phase: usize, mag: &[u16], pa: usize) -> Option<(i32, Vec<u8>)> {
    let start = pa + 19 + try_phase / 5;

    if start + 20 > mag.len() {
        return None;
    }

    let mut pos = start;
    let mut phase = try_phase % 5;

    let mut msg = [0u8; MODES_LONG_MSG_BYTES];
    msg[0] = slice_byte(mag, &mut pos, &mut phase);
    let df = (msg[0] >> 3) as u32;

    let nbytes = modes_message_len_by_type(df);
    if nbytes == 0 {
        return None;
    }

    for byte in msg.iter_mut().take(nbytes).skip(1) {
        if pos + 20 > mag.len() {
            return None;
        }
        *byte = slice_byte(mag, &mut pos, &mut phase);
    }

    let msgbits = nbytes * 8;
    let score = score_modes_message(&msg[..nbytes], msgbits);
    if score >= 0 {
        let trimmed = msg[..nbytes].to_vec();
        Some((score, trimmed))
    } else {
        None
    }
}

pub fn score_modes_message(msg: &[u8], msgbits: usize) -> i32 {
    let df = (msg[0] >> 3) as u32;

    if msgbits == 56 {
        if (VALID_DF_SHORT & (1 << df)) == 0 {
            return -2;
        }
    } else {
        if (VALID_DF_LONG & (1 << df)) == 0 {
            return -2;
        }
    }

    let syndrome = modes_checksum(msg, msgbits);
    let engine = if msgbits == 112 { &*CRC_ENGINE_112 } else { &*CRC_ENGINE_56 };

    let is_ap_df = matches!(df, 0 | 4 | 5 | 16 | 20 | 21);
    if is_ap_df {
        if syndrome == 0 {
            let icao = ((msg[1] as u32) << 16) | ((msg[2] as u32) << 8) | (msg[3] as u32);
            if crate::demod::icao_filter::icao_filter_test(icao) {
                return 1000;
            }
            return 700;
        }
        if let Some(info) = engine.diagnose(syndrome) {
            let icao = ((msg[1] as u32) << 16) | ((msg[2] as u32) << 8) | (msg[3] as u32);
            let known = crate::demod::icao_filter::icao_filter_test(icao);
            match info.errors {
                1 => return if known { 700 } else { 500 },
                _ => return 100,
            }
        }
        return 100;
    }

    if df == 11 {
        let icao = ((msg[1] as u32) << 16) | ((msg[2] as u32) << 8) | (msg[3] as u32);
        if syndrome == 0 {
            let iid = msg[0] & 0x07;
            if iid == 0 && crate::demod::icao_filter::icao_filter_test(icao) {
                return 1600;
            }
            if iid == 0 {
                return 750;
            }
            let known = crate::demod::icao_filter::icao_filter_test(icao);
            return if known { 700 } else { 500 };
        }
        if let Some(info) = engine.diagnose(syndrome) {
            if info.errors <= 1 && crate::demod::icao_filter::icao_filter_test(icao) {
                return 800;
            }
            if info.errors <= 1 {
                return 400;
            }
        }
        return 100;
    }

    if df == 17 || df == 18 {
        let icao = ((msg[1] as u32) << 16) | ((msg[2] as u32) << 8) | (msg[3] as u32);
        let known = crate::demod::icao_filter::icao_filter_test(icao);
        if syndrome == 0 {
            return if known { 1800 } else { 1400 };
        }
        if let Some(info) = engine.diagnose(syndrome) {
            match info.errors {
                1 => return if known { 900 } else { 700 },
                2 => return if known { 450 } else { 350 },
                _ => return 100,
            }
        }
        return 100;
    }

    if df == 19 || df == 24 {
        if syndrome == 0 {
            return 1000;
        }
        return 100;
    }

    -2
}

pub fn check_preamble(mag: &[u16], _threshold: u32) -> bool {
    mag.len() >= 16
        && mag[1] > mag[7]
        && mag[12] > mag[14]
        && mag[12] > mag[15]
}

#[allow(unused_assignments)]
pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32) -> Vec<(Vec<u8>, f64)> {
    let mut messages: Vec<(Vec<u8>, f64)> = Vec::new();
    let mut pa: usize = 0;
    let stop = mag_len.saturating_sub(MODES_LONG_MSG_SAMPLES + 16);

    while pa < stop && pa + 18 < mag_len {
        // Fast pre-check from readsb-C:
        // Checks that pa[1] > pa[7] and pa[12] > pa[14] and pa[12] > pa[15]
        // Each step advances pa by 1, testing the condition at the new position
        let mut pre_found = false;
        let pre_max = (stop - pa).min(10);
        for _ in 0..pre_max {
            if pa + 15 < mag_len
                && mag[pa + 1] > mag[pa + 7]
                && mag[pa + 12] > mag[pa + 14]
                && mag[pa + 12] > mag[pa + 15]
            {
                pre_found = true;
                break;
            }
            pa += 1;
        }
        if pa >= stop || !pre_found {
            if !pre_found { pa += 1; }
            continue;
        }
        if pa >= stop || pa + 18 >= mag_len {
            pa += 1;
            continue;
        }

        // Noise-based reference level (readsb-C: 5 noise samples)
        let base_noise = mag[pa + 5] as u32 + mag[pa + 8] as u32
            + mag[pa + 16] as u32 + mag[pa + 17] as u32 + mag[pa + 18] as u32;
        let ref_level = (base_noise * preamble_threshold) >> 5;

        let mut best_score: i32 = -42;
        let mut best_msg: Option<Vec<u8>> = None;

        let diff_2_3 = mag[pa + 2] as i32 - mag[pa + 3] as i32;
        let sum_1_4 = mag[pa + 1] as i32 + mag[pa + 4] as i32;
        let diff_10_11 = mag[pa + 10] as i32 - mag[pa + 11] as i32;
        let common3456 = sum_1_4 - diff_2_3 + mag[pa + 9] as i32 + mag[pa + 12] as i32;

        // Phase 3,4: peaks at 1,3,9,11-12 → pa_mag = common3456 - diff_10_11
        let pa_mag_34 = (common3456 - diff_10_11) as u32;
        if pa_mag_34 >= ref_level {
            for try_phase in 4..=5 {
                if let Some((score, msg)) = score_phase(try_phase, mag, pa) {
                    if score > best_score {
                        best_score = score;
                        best_msg = Some(msg);
                    }
                }
            }
        }

        // Phase 5,6: peaks at 1,3-4,9-10,12 → pa_mag = common3456 + diff_10_11
        let pa_mag_56 = (common3456 + diff_10_11) as u32;
        if pa_mag_56 >= ref_level {
            for try_phase in 6..=7 {
                if let Some((score, msg)) = score_phase(try_phase, mag, pa) {
                    if score > best_score {
                        best_score = score;
                        best_msg = Some(msg);
                    }
                }
            }
        }

        // Phase 7: peaks at 1-2,4,10,12 → pa_mag = sum_1_4 + 2*diff_2_3 + diff_10_11 + pa[12]
        let pa_mag_7 = (sum_1_4 + 2 * diff_2_3 + diff_10_11 + mag[pa + 12] as i32) as u32;
        if pa_mag_7 >= ref_level {
            if let Some((score, msg)) = score_phase(8, mag, pa) {
                if score > best_score {
                    best_score = score;
                    best_msg = Some(msg);
                }
            }
        }

        if let Some(msg) = best_msg {
            let signal = (mag[pa] as f64 + mag[pa + 2] as f64 + mag[pa + 7] as f64 + mag[pa + 9] as f64) / 4.0;
            let advance = msg.len() * 2;
            messages.push((msg, signal));
            pa += advance;
        } else {
            pa += 1;
        }
    }

    messages
}

pub fn demodulate2400_v2(mag: &[u16], count: usize, config: &DemodConfig) -> DemodResult {
    let messages = demodulate2400(mag, count, config.preamble_threshold)
        .into_iter()
        .map(|(bytes, signal)| Message { bytes, signal })
        .collect();
    DemodResult {
        messages,
        stats: MagBufStats::default(),
    }
}
