/// Generate synthetic Mode-S burst as SC16Q11 I/Q samples.
/// Creates a valid preamble at sample 0 + DF17 message
/// "8D4840D6202CC371C32CE0576098" as Manchester-coded symbols.
///
/// The preamble pattern uses pulses at samples 0, 2, 7, 9 (2 MHz timing:
/// 0, 1.0, 3.5, 4.5 microseconds) which matches the original dump1090
/// relative-comparison preamble detection.
fn main() {
    let msg_hex = "8D4840D6202CC371C32CE0576098";
    let msg: Vec<u8> = (0..msg_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&msg_hex[i..i+2], 16).unwrap())
        .collect();

    let preamble_samples = 16;
    let bits = msg.len() * 8;
    let msg_samples = bits * 2; // 2 samples per Manchester symbol

    let total_len = (preamble_samples + msg_samples + 1) as usize; // +1 guard sample for decoder window
    let mut iq = Vec::with_capacity(total_len * 4); // SC16Q11 = 4 bytes per sample

    // Strong pulse vs weak background for relative comparisons
    let pulse_i = 30000i16;
    let pulse_q = 10000i16;
    let quiet_i = 2000i16;
    let quiet_q = 500i16;

    // Generate preamble: pulses at indices 0, 2, 7, 9 (the 4 ADS-B pulses at 0, 1.0, 3.5, 4.5 us)
    // with quiet/gap at all other positions.
    for i in 0..preamble_samples {
        let (i_val, q_val) = match i {
            0 | 2 | 7 | 9 => (pulse_i, pulse_q),
            _ => (quiet_i, quiet_q),
        };
        iq.extend_from_slice(&i_val.to_le_bytes());
        iq.extend_from_slice(&q_val.to_le_bytes());
    }

    // Generate Manchester-coded message
    for byte in &msg {
        for bit_idx in (0..8).rev() {
            let bit = (byte >> bit_idx) & 1;
            // Manchester: 1 = high-low, 0 = low-high
            let samples = if bit == 1 {
                [(pulse_i, pulse_q), (quiet_i, quiet_q)]
            } else {
                [(quiet_i, quiet_q), (pulse_i, pulse_q)]
            };
            for &(i_val, q_val) in &samples {
                iq.extend_from_slice(&i_val.to_le_bytes());
                iq.extend_from_slice(&q_val.to_le_bytes());
            }
        }
    }

    // Add one guard sample for the decoder's 3-sample window at the last bit
    iq.extend_from_slice(&quiet_i.to_le_bytes());
    iq.extend_from_slice(&quiet_q.to_le_bytes());

    let path = std::path::Path::new("test_fixtures/synthetic_df17.iq");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &iq).unwrap();
    eprintln!("Wrote {} bytes ({} samples) to {}",
        iq.len(), iq.len() / 4, path.display());
}
