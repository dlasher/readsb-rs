const CRC_POLYNOMIAL: u32 = 0xFFF409;

/// Compute the 24-bit CRC of a Mode S message.
/// Matches `modesChecksum()` in crc.c exactly.
pub fn modes_checksum(msg: &[u8], bitlen: usize) -> u32 {
    let mut crc: u32 = 0;
    let nbytes = (bitlen + 7) / 8;
    for i in 0..nbytes {
        let byte = msg[i];
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
    crc & 0x00FFFFFF
}
