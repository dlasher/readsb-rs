use readsb::crc::modes_checksum;

// Known-good DF17 long message (valid CRC, C reference confirms 0x000000)
#[test]
fn test_crc_valid_long_message() {
    let msg: [u8; 14] = [0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                          0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    assert_eq!(modes_checksum(&msg, 112), 0x000000);
}

// Valid DF11 short message (CRC computed from C reference for first 4 bytes 0x5D4840D6)
#[test]
fn test_crc_valid_short_message() {
    let msg: [u8; 7] = [0x5D, 0x48, 0x40, 0xD6, 0xF8, 0x74, 0x0F];
    assert_eq!(modes_checksum(&msg, 56), 0x000000);
}

// Corrupted version of the valid long message
#[test]
fn test_crc_invalid_message() {
    let mut msg = [0x8Du8, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                   0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    msg[0] ^= 0x01;
    // Must be non-zero (basic corruption detection)
    assert_ne!(modes_checksum(&msg, 112), 0x000000);
}

// Cross-validate non-zero CRC against the C reference value.
// The C reference (crctests / crc.c) returns 0x587178 for this
// specific 1-bit error at position 0.
#[test]
fn test_crc_corrupted_exact_value() {
    let mut msg = [0x8Du8, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                   0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    msg[0] ^= 0x01;
    assert_eq!(modes_checksum(&msg, 112), 0x587178);
}

// All-zero message (CRC of zero should be zero)
#[test]
fn test_crc_all_zeros() {
    assert_eq!(modes_checksum(&[0u8; 14], 112), 0x000000);
}
