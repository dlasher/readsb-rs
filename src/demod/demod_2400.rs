#[inline(always)]
fn slice_phase0(m: &[u16]) -> i32 { 18 * m[0] as i32 - 15 * m[1] as i32 - 3 * m[2] as i32 }
#[inline(always)]
fn slice_phase1(m: &[u16]) -> i32 { 14 * m[0] as i32 - 5 * m[1] as i32 - 9 * m[2] as i32 }
#[inline(always)]
fn slice_phase2(m: &[u16]) -> i32 { 10 * m[0] as i32 + 5 * m[1] as i32 - 15 * m[2] as i32 }
#[inline(always)]
fn slice_phase3(m: &[u16]) -> i32 { 6 * m[0] as i32 + 15 * m[1] as i32 - 21 * m[2] as i32 }
#[inline(always)]
fn slice_phase4(m: &[u16]) -> i32 { 2 * m[0] as i32 + 25 * m[1] as i32 - 27 * m[2] as i32 }

const MODES_PREAMBLE_SAMPLES: usize = 16;
const MODES_LONG_MSG_SAMPLES: usize = 224;
const MODES_SHORT_MSG_SAMPLES: usize = 112;

pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32) -> Vec<(Vec<u8>, f64)> {
    let mut messages = Vec::new();
    let mut i = 0;
    while i + MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES <= mag_len {
        if check_preamble(&mag[i..], preamble_threshold) {
            if let Some(msg) = decode_message(&mag[i..], mag_len - i, 112) {
                messages.push(msg);
                i += MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES;
                continue;
            }
            if let Some(msg) = decode_message(&mag[i..], mag_len - i, 56) {
                messages.push(msg);
                i += MODES_PREAMBLE_SAMPLES + MODES_SHORT_MSG_SAMPLES;
                continue;
            }
        }
        i += 1;
    }
    messages
}

fn check_preamble(mag: &[u16], _threshold: u32) -> bool {
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

fn decode_message(mag: &[u16], mag_len: usize, bitlen: usize) -> Option<(Vec<u8>, f64)> {
    let sample_count = bitlen * 2;
    if mag_len < sample_count { return None; }
    let mut msg = vec![0u8; bitlen.div_ceil(8)];

    // Signal level = average of 4 preamble peak magnitudes
    let signal = (mag[0] as f64 + mag[2] as f64 + mag[7] as f64 + mag[9] as f64) / 4.0;

    let mut phase: i32 = 0;
    for bit in 0..bitlen {
        let sample_idx = MODES_PREAMBLE_SAMPLES + bit * 2;
        if sample_idx + 3 > mag_len { return None; }
        let slice = &mag[sample_idx..];
        let bit_val = match phase {
            0 => slice_phase0(slice) > 0,
            1 => slice_phase1(slice) > 0,
            2 => slice_phase2(slice) > 0,
            3 => slice_phase3(slice) > 0,
            4 => slice_phase4(slice) > 0,
            _ => slice_phase0(slice) > 0,
        };
        let byte_idx = bit / 8;
        let bit_idx = 7 - (bit % 8);
        if bit_val { msg[byte_idx] |= 1 << bit_idx; }
        phase += 6;
        if phase >= 30 { phase -= 30; }
    }
    Some((msg, signal))
}
