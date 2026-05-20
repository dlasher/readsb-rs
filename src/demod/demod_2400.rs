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

pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut i = 0;
    while i + MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES < mag_len {
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

fn check_preamble(mag: &[u16], threshold: u32) -> bool {
    let t = threshold as u16;
    mag.len() >= 20
        && mag[0] > t && mag[4] > t && mag[14] > t && mag[18] > t
        && mag[2] < t && mag[10] < t
}

fn decode_message(mag: &[u16], mag_len: usize, bitlen: usize) -> Option<Vec<u8>> {
    let sample_count = bitlen * 2;
    if mag_len < sample_count { return None; }
    let mut msg = vec![0u8; (bitlen + 7) / 8];
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
            _ => false,
        };
        let byte_idx = bit / 8;
        let bit_idx = 7 - (bit % 8);
        if bit_val { msg[byte_idx] |= 1 << bit_idx; }
        phase += 6;
        if phase >= 30 { phase -= 30; }
    }
    Some(msg)
}
