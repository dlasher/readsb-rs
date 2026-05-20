use readsb::crc::{modes_checksum, CrcFixEngine};

fn valid_msg() -> [u8; 14] {
    [
        0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
    ]
}

/// Single-bit error: flip bit 5.
/// bit 5: byte_idx = 5/8 = 0, bit_in_byte = 5, pos = 7 - 5 = 2 → msg[0] bit 2
#[test]
fn test_single_bit_error_correction() {
    let mut msg = valid_msg();
    // Flip bit 5 (bit 2 in byte 0)
    msg[0] ^= 1 << 2;

    let crc = modes_checksum(&msg, 112);
    assert_ne!(crc, 0, "Corrupted message must have non-zero CRC");

    let engine = CrcFixEngine::new(112);
    let info = engine.diagnose(crc).expect("Should find single-bit error");
    assert_eq!(info.errors, 1, "Should be a single-bit error");

    // Fix it
    let mut fixed = msg;
    CrcFixEngine::fix(&mut fixed, info);
    assert_eq!(
        modes_checksum(&fixed, 112),
        0,
        "Fixed message must have zero CRC"
    );
}

/// Double-bit error: flip bits 10 and 50.
/// bit 10: byte_idx=1, bit_in_byte=2, pos=7-2=5 → msg[1] bit 5
/// bit 50: byte_idx=6, bit_in_byte=2, pos=7-2=5 → msg[6] bit 5
#[test]
fn test_double_bit_error_correction() {
    let mut msg = valid_msg();
    msg[1] ^= 1 << 5; // bit 10
    msg[6] ^= 1 << 5; // bit 50

    let crc = modes_checksum(&msg, 112);
    let engine = CrcFixEngine::new(112);
    let info = engine.diagnose(crc).expect("Should find double-bit error");
    assert_eq!(info.errors, 2);

    let mut fixed = msg;
    CrcFixEngine::fix(&mut fixed, info);
    assert_eq!(modes_checksum(&fixed, 112), 0);
}

/// Uncorrectable error (three bits flipped) → diagnose should return None or a wrong fix.
/// At minimum it should not panic; the fix won't yield zero CRC.
#[test]
fn test_uncorrectable_error() {
    let mut msg = valid_msg();
    msg[0] ^= 1 << 2;
    msg[1] ^= 1 << 5;
    msg[2] ^= 1 << 3; // three bits

    let engine = CrcFixEngine::new(112);
    let crc = modes_checksum(&msg, 112);
    // Might find a spurious match — that's fine for a 2-bit corrector.
    // The important thing is it doesn't panic.
    if let Some(info) = engine.diagnose(crc) {
        // Attempting fix should not crash
        let mut fixed = msg;
        CrcFixEngine::fix(&mut fixed, info);
        // CRC will likely still be non-zero (uncorrectable)
        // But we just ensure it doesn't crash
    }
}
