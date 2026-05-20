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
