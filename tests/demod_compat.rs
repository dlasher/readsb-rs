use readsb::demod::{convert_to_magnitude, InputFormat};

#[test]
fn test_convert_sc16q11() {
    let input: [u8; 4] = [0x00, 0x08, 0x00, 0x00];
    let mut output = [0u16; 1];
    let count = convert_to_magnitude(&input, InputFormat::SC16Q11, &mut output);
    assert_eq!(count, 1);
    assert!(output[0] > 0);
}

#[test]
fn test_convert_u8() {
    let input: [u8; 4] = [200, 100, 200, 100];
    let mut output = [0u16; 2];
    let count = convert_to_magnitude(&input, InputFormat::U8, &mut output);
    assert_eq!(count, 2);
    assert!(output[0] > 0);
}

#[test]
fn test_convert_empty() {
    let mut output = [0u16; 1];
    let count = convert_to_magnitude(&[], InputFormat::SC16Q11, &mut output);
    assert_eq!(count, 0);
}

// ===== Demodulator tests =====

use readsb::demod::demodulate2400;

#[test]
fn test_demod_no_messages_in_noise() {
    let noise: Vec<u16> = vec![100; 1000];
    let msgs = demodulate2400(&noise, noise.len(), 32768);
    assert!(msgs.is_empty());
}

#[test]
fn test_demod_empty_buffer() {
    let msgs = demodulate2400(&[], 0, 32768);
    assert!(msgs.is_empty());
}

#[test]
fn test_demod_small_buffer() {
    let small = [100u16; 50];
    let msgs = demodulate2400(&small, small.len(), 32768);
    assert!(msgs.is_empty());
}

#[test]
fn test_demod_signal_level() {
    // Valid preamble: pulses at samples 0, 2, 7, 9 with magnitude 5000
    let preamble = [
        5000, 50, 5000, 50, 50, 50, 50,
        5000, 50, 5000, 50, 50, 50, 50,
        50, 50,
    ];
    // 112 bits of Manchester-encoded follow (224 samples) + margin for phase alignment
    let padding = vec![200u16; 256];
    let mut buffer: Vec<u16> = Vec::with_capacity(preamble.len() + 256);
    buffer.extend_from_slice(&preamble);
    buffer.extend_from_slice(&padding);

    let result = demodulate2400(&buffer, buffer.len(), 0);
    assert_eq!(result.len(), 1, "Should detect one message");
    let (msg, signal) = &result[0];
    // Signal should be average of 4 preamble peaks
    assert!((signal - 5000.0).abs() < 1.0, "Signal should be ~5000");
    assert_eq!(msg.len(), 14, "Should decode as long (112-bit) frame");
}
