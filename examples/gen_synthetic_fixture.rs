/// Generate synthetic Mode-S burst as SC16Q11 I/Q samples.
/// Creates a valid preamble at sample 0 + DF17 message
/// "8D4840D6202CC371C32CE0576098" as Manchester-coded symbols.
fn main() {
    let msg_hex = "8D4840D6202CC371C32CE0576098";
    let msg: Vec<u8> = (0..msg_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&msg_hex[i..i+2], 16).unwrap())
        .collect();

    // Mode-S preamble: pulses at sample positions 0, 2, 7, 9
    // Each pulse is ~0.5us at 2.4MHz = ~1.2 samples = let's use 1 sample per pulse
    // Preamble pattern (16 samples at 2.4MHz/bit = 2 samples per bit):
    // 1 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0
    // (pulses at positions 0, 2, 7, 9 in the 16-sample preamble)
    // But this needs to be 0.5us pulse width. At 2.4MHz, one sample = 0.417us
    // So 1 sample pulse ≈ 0.417us. Close enough for detection.

    let _sample_rate = 2_400_000u32;
    let preamble_samples = 16;
    let bits = msg.len() * 8;
    let msg_samples = bits * 2; // 2 samples per Manchester symbol

    let total_len = (preamble_samples + msg_samples) as usize;
    let mut iq = Vec::with_capacity(total_len * 4); // SC16Q11 = 4 bytes per sample

    // Generate preamble: pulses at indices 0, 2, 7, 9 with quiet at 1, 5
    for i in 0..preamble_samples {
        let amplitude: f32 = match i {
            0 | 2 | 7 | 9 => 0.7,  // Pulse
            _ => 0.01,              // Quiet (but not zero to avoid I/Q DC)
        };
        // I/Q with small phase offset for realism
        let i_val = (amplitude * 2047.0) as i16;
        let q_val = (amplitude * 100.0) as i16;
        iq.extend_from_slice(&i_val.to_le_bytes());
        iq.extend_from_slice(&q_val.to_le_bytes());
    }

    // Generate Manchester-coded message
    for byte in &msg {
        for bit_idx in (0..8).rev() {
            let bit = (byte >> bit_idx) & 1;
            // Manchester: 1 = high-low, 0 = low-high
            let (s1, s2) = if bit == 1 {
                (0.8, 0.01)
            } else {
                (0.01, 0.8)
            };
            // Two samples per symbol
            for &sample_amp in &[s1, s2] {
                let i_val = (sample_amp * 2047.0) as i16;
                let q_val = (sample_amp * 100.0) as i16;
                iq.extend_from_slice(&i_val.to_le_bytes());
                iq.extend_from_slice(&q_val.to_le_bytes());
            }
        }
    }

    let path = std::path::Path::new("test_fixtures/synthetic_df17.iq");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &iq).unwrap();
    eprintln!("Wrote {} bytes ({} samples) to {}",
        iq.len(), iq.len() / 4, path.display());
}
