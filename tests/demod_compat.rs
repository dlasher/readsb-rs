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
