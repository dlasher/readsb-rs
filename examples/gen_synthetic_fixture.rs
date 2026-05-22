/// Generate synthetic Mode-S burst as SC16Q11 I/Q samples.
/// Encoded at 2.4 MHz slice_byte timing: preamble (16) + gap (3) + data bytes.
fn main() {
    let msg_hex = "8D4840D6202CC371C32CE0576098";
    let msg: Vec<u8> = (0..msg_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&msg_hex[i..i+2], 16).unwrap())
        .collect();

    let preamble_len = 16;
    let gap = 3; // score_phase starts reading at pa + 19, not pa + 16

    // Compute data position: each byte advances 19/20 samples via slice_byte phase
    let mut data_start = preamble_len + gap;
    let mut phase = 0usize;
    let mut byte_positions = Vec::new();
    for _ in &msg {
        byte_positions.push(data_start);
        data_start += if phase == 4 { 20 } else { 19 };
        phase = (phase + 1) % 5;
    }
    let total_samples = data_start;

    let offsets = [0usize, 2, 4, 7, 9, 12, 14, 16];
    let mut mag = vec![0u16; total_samples];

    // Preamble: pulses at 0, 2, 7, 9
    for (i, m) in mag.iter_mut().enumerate().take(preamble_len) {
        *m = match i { 0 | 2 | 7 | 9 => 31623, _ => 2062 };
    }

    // Encode bytes. All slice functions compare m[0] (positive) vs m[2] (negative).
    // bit=1: set m[0] high, bit=0: set m[2] high (coefficient -20, -18, -15, -9, or -3).
    let mut byte_phase = 0usize;
    for (byte_idx, &byte) in msg.iter().enumerate() {
        let base = byte_positions[byte_idx];
        for (bit, &off) in offsets.iter().enumerate() {
            let bit_val = (byte >> (7 - bit)) & 1;
            if bit_val == 1 {
                mag[base + off] = 31623;
            } else {
                mag[base + off + 2] = 31623;
            }
        }
        byte_phase = (byte_phase + 1) % 5;
    }

    // Debug: verify first byte decodes correctly
    let slices: [fn(&[u16]) -> i32; 5] = [
        |m| 18*m[0] as i32 - 15*m[1] as i32 - 3*m[2] as i32,
        |m| 14*m[0] as i32 - 5*m[1] as i32 - 9*m[2] as i32,
        |m| 16*m[0] as i32 + 5*m[1] as i32 - 20*m[2] as i32,
        |m| 7*m[0] as i32 + 11*m[1] as i32 - 18*m[2] as i32,
        |m| 4*m[0] as i32 + 15*m[1] as i32 - 20*m[2] as i32 + m[3] as i32,
    ];
    let test_pos = preamble_len + gap;
    let test_phase = 0usize;
    let mut test_byte = 0u8;
    for (bit, &off) in offsets.iter().enumerate() {
        let func_idx = (test_phase + bit * 2) % 5;
        let result = slices[func_idx](&mag[test_pos + off..]);
        if result > 0 { test_byte |= 1 << (7 - bit); }
        eprintln!("  bit={} func={} off={} result={:6} -> {}", bit, func_idx, off, result, if result > 0 {'1'} else {'0'});
    }
    eprintln!("First byte: 0x{:02X} (expected 0x{:02X})", test_byte, msg[0]);
    assert_eq!(test_byte, msg[0], "Fixture encoding must produce correct first byte");

    // Convert magnitude buffer to SC16Q11 I/Q
    let mut iq = Vec::with_capacity(total_samples * 4);
    for &m in &mag {
        let i_val = (m as f32 * 0.95).round() as i16;
        let q_val = (m as f32 * 0.316).round() as i16;
        iq.extend_from_slice(&i_val.to_le_bytes());
        iq.extend_from_slice(&q_val.to_le_bytes());
    }

    let path = std::path::Path::new("test_fixtures/synthetic_df17.iq");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &iq).unwrap();
    eprintln!("Wrote {} bytes ({} samples) to {}",
        iq.len(), iq.len() / 4, path.display());
}
