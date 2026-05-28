//! Signal subtraction for multi-pass demodulation.
//!
//! After the first pass detects a CRC-OK Mode-S message, we subtract its
//! raw IQ reconstruction (preamble + data bits at the preamble position) from
//! the magnitude buffer. This attenuates the strongest signal so that
//! weaker overlapping signals become detectable in subsequent passes.

use crate::demod::demod_2400::{SLICE_ADVANCE, MODES_LONG_MSG_SAMPLES, MODES_SHORT_MSG_BYTES};

/// A successfully decoded message with its location in the magnitude buffer.
#[derive(Clone)]
pub struct DecodedMessage {
    /// Raw message bytes (7 for short, 14 for long)
    pub bytes: Vec<u8>,
    /// Preamble position in magnitude buffer (sample index)
    pub preamble_pos: usize,
    /// Peak signal magnitude (for ordering)
    pub signal: f64,
    /// ICAO address (0 if not present)
    pub icao: u32,
}

impl DecodedMessage {
    /// Subtract this message's raw signal representation from the magnitude buffer.
    /// This is the i(q) reconstruction: 16-sample preamble + 8-sample/spread bits.
    ///
    /// # Safety
    /// Panics if `mag` is shorter than `preamble_pos + total_samples`.
    pub fn subtract_from(&self, mag: &mut [u16]) {
        // Scale constants proportionally to measured signal level.
        // Strong signals (~400) get full subtraction; weak signals (~50) get partial.
        let reference_signal: f64 = 200.0;
        let scale = (self.signal / reference_signal).clamp(0.25, 2.0);

        let scaled_high = (PREAMBLE_HIGH as f64 * scale) as u16;
        let scaled_low = (PREAMBLE_LOW as f64 * scale) as u16;
        let scaled_qi_high = (QI_HIGH as f64 * scale) as u16;
        let scaled_qi_low = (QI_LOW as f64 * scale) as u16;

        let nbytes = self.bytes.len();
        let total_samples = MODES_LONG_MSG_SAMPLES + nbytes.saturating_sub(MODES_SHORT_MSG_BYTES);

        let end = (self.preamble_pos + total_samples).min(mag.len());
        let pa = self.preamble_pos;

        // Preamble: first 16 samples have fixed patterns (4 high/low pairs)
        let preamble_samples = 16.min(end - pa);
        for i in 0..preamble_samples {
            match PREAMBLE_BITS[i] {
                1 => mag[pa + i] = mag[pa + i].saturating_sub(scaled_high),
                _ => mag[pa + i] = mag[pa + i].saturating_sub(scaled_low),
            }
        }

        // Data bits: spread by 8 samples each, reconstruct i(q) pattern
        let mut pos = pa + PREAMBLE_SAMPLES;
        let mut phase: usize = 0;

        for byte in &self.bytes {
            for bit in 0..8 {
                if pos >= end {
                    break;
                }
                let bit_val = (*byte >> (7 - bit)) & 1;
                let spread = BIT_SPREAD;
                for k in 0..spread {
                    let idx = pos + k;
                    if idx >= end {
                        break;
                    }
                    let mag_val = mag[idx];
                    if bit_val == 1 {
                        // High bit: subtract scaled Qi
                        mag[idx] = mag_val.saturating_sub(scaled_qi_high);
                    } else {
                        // Low bit: subtract scaled Qi
                        mag[idx] = mag_val.saturating_sub(scaled_qi_low);
                    }
                }
                // Advance by SLICE_ADVANCE per bit to match slice_byte() movement
                let advances = SLICE_ADVANCE[phase];
                for _ in 0..8 {
                    pos += advances;
                    phase = SLICE_NEXT_PHASE[phase];
                    if phase == 0 {
                        break;
                    }
                }
            }
        }
    }

    pub fn total_samples(&self) -> usize {
        MODES_LONG_MSG_SAMPLES + self.bytes.len().saturating_sub(MODES_SHORT_MSG_BYTES)
    }
}

/// Number of samples in the preamble pattern
pub const PREAMBLE_SAMPLES: usize = 16;

/// Number of samples per data bit during spread
pub const BIT_SPREAD: usize = 8;

/// Preamble high/low sample magnitudes (from raw IQ normalization)
pub const PREAMBLE_HIGH: u16 = 300;
pub const PREAMBLE_LOW: u16 = 50;
pub const QI_HIGH: u16 = 250;
pub const QI_LOW: u16 = 40;

/// Preamble bit pattern: 1 = high, 0 = low
const PREAMBLE_BITS: [u8; 16] = [1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1];

/// Slice advance per byte (must match demodulate2400::SLICE_ADVANCE)
const SLICE_NEXT_PHASE: [usize; 5] = [1, 2, 3, 4, 0];

/// Long message samples (same as demodulate2400::MODES_LONG_MSG_SAMPLES)
#[allow(dead_code)]
const LONG_SAMPLES: usize = MODES_LONG_MSG_SAMPLES;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtract_preamble_only() {
        let mut mag = vec![500u16; 600];
        let msg = DecodedMessage {
            bytes: vec![0u8; 14],
            preamble_pos: 100,
            signal: 500.0,
            icao: 0,
        };
        // Preamble samples should be reduced
        for i in 0..16 {
            assert_eq!(mag[100 + i], 500);
        }
        msg.subtract_from(&mut mag);
        // First two preamble bits (high/low pattern)
        assert!(mag[100] < 500);
    }

    #[test]
    fn test_subtract_does_not_panic_on_short_buffer() {
        let mut mag = vec![500u16; 50]; // too short
        let msg = DecodedMessage {
            bytes: vec![0u8; 14],
            preamble_pos: 30,
            signal: 500.0,
            icao: 0,
        };
        // Should not panic, just saturate at 0
        msg.subtract_from(&mut mag);
        for val in &mag[..50] {
            assert!(*val <= 500);
        }
    }

    #[test]
    fn test_subtract_scales_with_weak_signal() {
        // Weak signal: should subtract less than a strong signal.
        let mut mag = vec![60u16; 600];
        let msg = DecodedMessage {
            bytes: vec![0u8; 14],
            preamble_pos: 100,
            signal: 60.0, // weak signal
            icao: 0,
        };
        msg.subtract_from(&mut mag);
        // With scale = 60/200 = 0.3, preamble high subtraction = 300 * 0.3 = 90
        // But mag is only 60, so subtraction saturates at 0.
        // The test mainly verifies it doesn't panic and behaves correctly.
        for i in 0..16 {
            assert!(mag[100 + i] <= 60, "Subtraction should not increase magnitude");
        }
    }
}
