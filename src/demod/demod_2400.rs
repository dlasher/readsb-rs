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
            preamble_threshold: 32768,
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

pub fn slice_byte(mag: &[u16], pos: &mut usize, phase: &mut usize) -> u8 {
    let slice_funcs: [fn(&[u16]) -> i32; 5] = [
        slice_phase0,
        slice_phase1,
        slice_phase2,
        slice_phase3,
        slice_phase4,
    ];
    let offsets = [0usize, 2, 4, 7, 9, 12, 14, 16];
    let mut byte = 0;
    let p = *pos;
    for (bit, &off) in offsets.iter().enumerate() {
        let func_idx = (*phase + bit * 2) % 5;
        let result = slice_funcs[func_idx](&mag[p + off..]);
        if result > 0 {
            byte |= 1 << (7 - bit);
        }
    }
    if *phase == 4 {
        *pos += 20;
    } else {
        *pos += 19;
    }
    *phase = (*phase + 1) % 5;
    byte
}

/// Compute scoring table for a decoded Mode-S message.
/// Matches readsb-C's scoreModesMessage() logic.
pub fn score_modes_message(msg: &[u8], msgbits: usize) -> i32 {
    let df = (msg[0] >> 3) as u32;

    // Valid DF bitsets matching readsb-C
    const DF_SHORT_OK: u32 = (1 << 0) | (1 << 4) | (1 << 5) | (1 << 11) | (1 << 20) | (1 << 21);
    const DF_LONG_OK: u32 = (1 << 16) | (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21) | (1 << 24);

    if msgbits == 56 {
        if (DF_SHORT_OK & (1 << df)) == 0 {
            return -2;
        }
    } else {
        if (DF_LONG_OK & (1 << df)) == 0 {
            return -2;
        }
    }

    let syndrome = modes_checksum(msg, msgbits);
    let engine = if msgbits == 112 { &*CRC_ENGINE_112 } else { &*CRC_ENGINE_56 };

    // Address-parity DFs (0/4/5 short, 16/20/21 long) — AP field = CRC = ICAO address
    let is_ap_df = matches!(df, 0 | 4 | 5 | 16 | 20 | 21);
    if is_ap_df {
        if syndrome == 0 {
            let icao = ((msg[1] as u32) << 16) | ((msg[2] as u32) << 8) | (msg[3] as u32);
            if crate::demod::icao_filter::icao_filter_test(icao) {
                return 1000;
            }
            return -1;
        }
        return -2;
    }

    // DF 11 — short frame, IID in CA field
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
        }
        if let Some(info) = engine.diagnose(syndrome) {
            if info.errors <= 1 && crate::demod::icao_filter::icao_filter_test(icao) {
                return 800;
            }
        }
        return -2;
    }

    // DF 17/18 — ADS-B long frames
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
                _ => return -2,
            }
        }
        return -2;
    }

    // DF 19, 24 — CRC only, no tiered scoring
    if df == 19 || df == 24 {
        if syndrome == 0 {
            return 1000;
        }
        return -2;
    }

    -2
}

const MODES_PREAMBLE_SAMPLES: usize = 16;
const MODES_LONG_MSG_SAMPLES: usize = 269;
const MODES_SHORT_MSG_SAMPLES: usize = 135;
const MODES_LONG_MSG_BYTES: usize = 14;
const MODES_SHORT_MSG_BYTES: usize = 7;

/// Try decoding a frame from magnitude buffer at `pa` using phase `try_phase`.
/// Returns the score (from score_modes_message) and the decoded bytes, or None.
pub fn score_phase(try_phase: usize, mag: &[u16], pa: usize, msgbits: usize) -> Option<(i32, Vec<u8>)> {
    let nbytes = if msgbits == 112 { MODES_LONG_MSG_BYTES } else { MODES_SHORT_MSG_BYTES };
    let start = pa + 19 + try_phase / 5;

    // Decode all bytes using slice_byte
    let mut pos = start;
    let mut phase = try_phase % 5;
    let mut msg = vec![0u8; nbytes];
    for byte in msg.iter_mut() {
        if pos + 20 > mag.len() {
            return None;
        }
        *byte = slice_byte(mag, &mut pos, &mut phase);
    }

    let score = score_modes_message(&msg, msgbits);
    if score >= 0 {
        Some((score, msg))
    } else {
        None
    }
}

pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32) -> Vec<(Vec<u8>, f64)> {
    let mut messages = Vec::new();
    let mut i = 0;
    while i + MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES <= mag_len {
        if check_preamble(&mag[i..], preamble_threshold) {
            let mut best_score = -1;
            let mut best_msg: Option<Vec<u8>> = None;
            let mut best_msgbits = 0;

            // Try 5 phases for long frame
            for try_phase in 0..5 {
                if let Some((score, msg)) = score_phase(try_phase, mag, i, 112) {
                    if score > best_score {
                        best_score = score;
                        best_msg = Some(msg);
                        best_msgbits = 112;
                    }
                }
            }
            // Try 5 phases for short frame
            if best_score < 0 {
                for try_phase in 0..5 {
                    if let Some((score, msg)) = score_phase(try_phase, mag, i, 56) {
                        if score > best_score {
                            best_score = score;
                            best_msg = Some(msg);
                            best_msgbits = 56;
                        }
                    }
                }
            }

            if let Some(msg) = best_msg {
                let signal = (mag[i] as f64 + mag[i + 2] as f64 + mag[i + 7] as f64 + mag[i + 9] as f64) / 4.0;
                messages.push((msg, signal));
                if best_msgbits == 112 {
                    i += MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES;
                } else {
                    i += MODES_PREAMBLE_SAMPLES + MODES_SHORT_MSG_SAMPLES;
                }
                continue;
            }
        }
        i += 1;
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

pub fn check_preamble(mag: &[u16], _threshold: u32) -> bool {
    if mag.len() < 20 { return false; }

    // First check: relative comparisons between the first 10 samples
    // as used by the original dump1090 preamble detection.
    //
    // At 2 MHz (0.5 µs/sample) the Mode-S preamble pulses are:
    //   0   - pulse 1
    //   2   - pulse 2
    //   7   - pulse 3
    //   9   - pulse 4
    //
    // Check relative relationships:
    //   mag[0] > mag[1]        pulse 1 > immediate gap
    //   mag[1] < mag[2]        gap < pulse 2
    //   mag[2] > mag[3]        pulse 2 > gap
    //   mag[3] < mag[0]        gap < pulse 1 (long range)
    //   mag[4..6] < mag[0]     mid-gap < pulse 1
    //   mag[7] > mag[8]        pulse 3 > gap
    //   mag[8] < mag[9]        gap < pulse 4
    //   mag[9] > mag[6]        pulse 4 > mid-gap
    if !(mag[0] > mag[1] &&
        mag[1] < mag[2] &&
        mag[2] > mag[3] &&
        mag[3] < mag[0] &&
        mag[4] < mag[0] &&
        mag[5] < mag[0] &&
        mag[6] < mag[0] &&
        mag[7] > mag[8] &&
        mag[8] < mag[9] &&
        mag[9] > mag[6])
    {
        return false;
    }

    // Second check: the gap between spikes must be low relative to pulse peaks.
    // The divisor 6 is from original: (sum_of_4_peaks) / 6
    let high = (mag[0] as u32 + mag[2] as u32 + mag[7] as u32 + mag[9] as u32) / 6;
    if mag[4] as u32 >= high || mag[5] as u32 >= high { return false; }
    if mag[11] as u32 >= high || mag[12] as u32 >= high ||
       mag[13] as u32 >= high || mag[14] as u32 >= high { return false; }

    true
}
