use std::collections::HashMap;

pub const MODES_MAX_BITERRORS: usize = 2;

pub struct ErrorInfo {
    pub syndrome: u32,
    pub errors: usize,
    pub bits: [i8; MODES_MAX_BITERRORS],
}

pub struct CrcFixEngine {
    table: HashMap<u32, ErrorInfo>,
    #[allow(dead_code)]
    max_bitlen: usize,
}

/// Precompute a syndrome table for single and double bit errors.
///
/// For each possible single-bit error position, compute the syndrome
/// by creating a message with that bit flipped and calling modes_checksum.
/// Then for each possible pair of bit positions, compute the syndrome.
///
/// This matches the approach in crc.c's modesChecksumInit function.
impl CrcFixEngine {
    pub fn new(max_bitlen: usize) -> Self {
        let mut table = HashMap::new();
        let nbytes = max_bitlen.div_ceil(8);

        // Single bit errors — skip bits 0-4 (DF type field)
        for bit_pos in 5..max_bitlen {
            let mut error_msg = vec![0u8; nbytes];
            let byte_idx = bit_pos / 8;
            let bit_idx = 7 - (bit_pos % 8);
            error_msg[byte_idx] |= 1 << bit_idx;

            let syndrome = super::engine::modes_checksum(&error_msg, max_bitlen);
            table.insert(
                syndrome,
                ErrorInfo {
                    syndrome,
                    errors: 1,
                    bits: [bit_pos as i8, -1],
                },
            );
        }

        // Double bit errors — skip bits 0-4 (DF type field)
        for bit1 in 5..max_bitlen {
            for bit2 in (bit1 + 1)..max_bitlen {
                let mut error_msg = vec![0u8; nbytes];
                let byte1 = bit1 / 8;
                let bit_idx1 = 7 - (bit1 % 8);
                error_msg[byte1] |= 1 << bit_idx1;
                let byte2 = bit2 / 8;
                let bit_idx2 = 7 - (bit2 % 8);
                error_msg[byte2] |= 1 << bit_idx2;
                let syndrome = super::engine::modes_checksum(&error_msg, max_bitlen);
                table.entry(syndrome).or_insert(ErrorInfo {
                            syndrome,
                            errors: 2,
                            bits: [bit1 as i8, bit2 as i8],
                        });
            }
        }

        CrcFixEngine {
            table,
            max_bitlen,
        }
    }

    pub fn diagnose(&self, syndrome: u32) -> Option<&ErrorInfo> {
        self.table.get(&syndrome)
    }

    pub fn fix(msg: &mut [u8], info: &ErrorInfo) {
        for &bit_pos in &info.bits {
            if bit_pos < 0 {
                break;
            }
            let byte_idx = bit_pos as usize / 8;
            let bit_idx = 7 - (bit_pos as usize % 8);
            msg[byte_idx] ^= 1 << bit_idx;
        }
    }
}
