const CRC_POLYNOMIAL: u32 = 0xFFF409;

/// Compute the 24-bit CRC syndrome of a Mode S message.
/// Matches `modesChecksum()` in crc.c exactly.
///
/// This is NOT a general CRC-24 over all bytes.  It processes
/// the first (n-3) data bytes through a bit-serial shift register,
/// then XORs the final 3 bytes (the embedded CRC/parity field).
/// The combined result is 0 for a valid message.
pub fn modes_checksum(msg: &[u8], bitlen: usize) -> u32 {
    let nbytes = bitlen.div_ceil(8);
    let databytes = nbytes.saturating_sub(3);

    // Process data bytes through the CRC shift register
    let mut crc: u32 = 0;
    for &byte in msg.iter().take(databytes) {
        for bitidx in 0..8 {
            let bitpos = 7 - bitidx;
            let msg_bit = (byte >> bitpos) & 1;
            if (crc & 0x800000) != ((msg_bit as u32) << 23) {
                crc = (crc << 1) ^ CRC_POLYNOMIAL;
            } else {
                crc <<= 1;
            }
        }
    }

    // XOR in the last 3 bytes (the embedded CRC/parity field)
    if nbytes >= 3 {
        crc ^= (msg[nbytes - 3] as u32) << 16
             | (msg[nbytes - 2] as u32) << 8
             | msg[nbytes - 1] as u32;
    }

    crc & 0x00FFFFFF
}
