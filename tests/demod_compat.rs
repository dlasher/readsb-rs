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

// ===== Slice function tests =====

use readsb::demod::{slice_phase0, slice_phase1, slice_phase2, slice_phase3, slice_phase4};

#[test]
fn test_slice_phase0_matches_readsb_c() {
    assert_eq!(slice_phase0(&[100, 200, 150]), -1650);
}

#[test]
fn test_slice_phase1_matches_readsb_c() {
    assert_eq!(slice_phase1(&[100, 200, 150]), -950);
}

#[test]
fn test_slice_phase2_matches_readsb_c() {
    // readsb-C: 16*m[0] + 5*m[1] - 20*m[2]
    // Current:  10*m[0] + 5*m[1] - 15*m[2]
    assert_eq!(slice_phase2(&[100, 200, 150]), -400);
}

#[test]
fn test_slice_phase3_matches_readsb_c() {
    // readsb-C: 7*m[0] + 11*m[1] - 18*m[2]
    // Current:  6*m[0] + 15*m[1] - 21*m[2]
    assert_eq!(slice_phase3(&[100, 200, 150]), 200);
}

#[test]
fn test_slice_phase4_matches_readsb_c() {
    // readsb-C: 4*m[0] + 15*m[1] - 20*m[2] + 1*m[3]
    // Current: 2*m[0] + 25*m[1] - 27*m[2]  (no m[3])
    assert_eq!(slice_phase4(&[100, 200, 150, 50]), 450);
}

// ===== slice_byte tests =====

use readsb::demod::slice_byte;

/// Construct a magnitude buffer that makes slice_byte(phase=0) produce 0xA5.
/// Each bit position uses specific slice function at specific offset:
///   bit7 @+0  (slice_phase0): m[0]>m[1],m[2]   → 100,0,0
///   bit6 @+2  (slice_phase2): m[2]>m[4]         → 0,0,100
///   bit5 @+4  (slice_phase4): m[4]>m[6],m[7]    → 100,0,0,0
///   bit4 @+7  (slice_phase1): m[7],m[8]<m[9]    → 0,0,100
///   bit3 @+9  (slice_phase3): m[9],m[10]<m[11]  → 0,0,100
///   bit2 @+12 (slice_phase0): m[12]>m[13],m[14] → 100,0,0
///   bit1 @+14 (slice_phase2): m[14],m[15]<m[16] → 0,0,100
///   bit0 @+16 (slice_phase4): m[16]>m[18],m[19] → 100,0,0,0
fn mag_for_byte(val: u8, offset: usize) -> [u16; 20] {
    let mut mag = [0u16; 20];
    // bit7 (MSB)
    if val & 0x80 != 0 { mag[offset] = 100; }
    else { mag[offset+2] = 100; }
    // bit6
    if val & 0x40 != 0 { mag[offset+2] = 100; }
    else { mag[offset+4] = 100; }
    // bit5
    if val & 0x20 != 0 { mag[offset+4] = 100; }
    else { mag[offset+7] = 100; }
    // bit4
    if val & 0x10 != 0 { mag[offset+7] = 100; }
    else { mag[offset+9] = 100; }
    // bit3
    if val & 0x08 != 0 { mag[offset+9] = 100; }
    else { mag[offset+11] = 100; }
    // bit2
    if val & 0x04 != 0 { mag[offset+12] = 100; }
    else { mag[offset+14] = 100; }
    // bit1
    if val & 0x02 != 0 { mag[offset+14] = 100; }
    else { mag[offset+16] = 100; }
    // bit0 (LSB)
    if val & 0x01 != 0 { mag[offset+16] = 100; }
    else { mag[offset+19] = 100; }
    mag
}

#[test]
fn test_slice_byte_phase0_decodes_0xa5() {
    let mag = mag_for_byte(0xA5, 0);
    let mut pos = 0;
    let mut phase = 0;
    let result = slice_byte(&mag, &mut pos, &mut phase);
    assert_eq!(result, 0xA5, "Phase 0 must decode 0xA5");
    assert_eq!(phase, 1, "Phase must advance to 1");
    assert_eq!(pos, 19, "Position must advance by 19");
}

// ===== ICAO filter tests =====

use readsb::demod::icao_filter::{icao_filter_add, icao_filter_test};

#[test]
fn test_icao_filter_empty() {
    icao_filter_add(0xAA0011);
    assert!(icao_filter_test(0xAA0011), "Added address must match");
    assert!(!icao_filter_test(0xA1B2C3), "Different address must not match");
}

#[test]
fn test_icao_filter_unknown() {
    assert!(!icao_filter_test(0xBB0022), "Unknown address must not match");
}

// ===== Preamble detection tests =====

use readsb::demod::check_preamble;

#[test]
fn test_preamble_detects_valid_alignment_phase0() {
    // Build a magnitude buffer with a preamble at phase 0 alignment.
    // Preamble pulses at samples 0, 2, 7, 9 (peak magnitude), gaps near zero.
    let preamble: [u16; 20] = [
        5000, 50, 5000, 50, 50, 50, 50,
        5000, 50, 5000, 50, 50, 50, 50,
        50, 50, 50, 50, 50, 50,
    ];
    assert!(check_preamble(&preamble, 0), "Valid preamble must be detected");
}

#[test]
fn test_preamble_rejects_noise() {
    let uniform_noise = [100u16; 20];
    assert!(!check_preamble(&uniform_noise, 0), "Uniform noise must be rejected");
}

#[test]
fn test_preamble_rejects_flatline() {
    let zeros = [0u16; 20];
    assert!(!check_preamble(&zeros, 0), "All-zero buffer must be rejected");
}

// ===== ScoreModesMessage tests =====

use readsb::crc::modes_checksum;
use readsb::demod::score_modes_message;

/// Build a valid 112-bit (14-byte) Mode-S frame with given contents.
/// Computes CRC such that modes_checksum returns 0.
fn make_df17_frame(icao: u32, me: &[u8; 7], ca: u8) -> Vec<u8> {
    let mut msg = vec![0u8; 14];
    msg[0] = (17 << 3) | (ca & 0x07);
    msg[1] = (icao >> 16) as u8;
    msg[2] = (icao >> 8) as u8;
    msg[3] = icao as u8;
    msg[4..11].copy_from_slice(me);
    // Set CRC field to 0, compute checksum, then set CRC = checksum so final result is 0
    let syndrome = modes_checksum(&msg, 112);
    msg[11] = (syndrome >> 16) as u8;
    msg[12] = (syndrome >> 8) as u8;
    msg[13] = syndrome as u8;
    msg
}

/// Build a valid 56-bit (7-byte) DF11 frame
fn make_df11_frame(icao: u32, iid: u8) -> Vec<u8> {
    let mut msg = vec![0u8; 7];
    msg[0] = (11 << 3) | (iid & 0x07);
    msg[1] = (icao >> 16) as u8;
    msg[2] = (icao >> 8) as u8;
    msg[3] = icao as u8;
    let syndrome = modes_checksum(&msg, 56);
    msg[4] = (syndrome >> 16) as u8;
    msg[5] = (syndrome >> 8) as u8;
    msg[6] = syndrome as u8;
    msg
}

#[test]
fn test_score_df17_good_unknown() {
    let frame = make_df17_frame(0xAAA001, &[0; 7], 5);
    let score = score_modes_message(&frame, 112);
    assert_eq!(score, 1400, "DF17, CRC=0, unknown ICAO should score 1400");
}

#[test]
fn test_score_df17_good_known() {
    let frame = make_df17_frame(0xBBB002, &[0; 7], 5);
    icao_filter_add(0xBBB002);
    let score = score_modes_message(&frame, 112);
    assert_eq!(score, 1800, "DF17, CRC=0, known ICAO should score 1800");
}

#[test]
fn test_score_df17_1bit_error_known() {
    let mut frame = make_df17_frame(0xCCCCCC, &[0; 7], 5);
    icao_filter_add(0xCCCCCC);
    frame[0] ^= 0x04;
    let score = score_modes_message(&frame, 112);
    assert_eq!(score, 900, "DF17, 1-bit error, known ICAO should score 900");
}

#[test]
fn test_score_df11_good() {
    let frame = make_df11_frame(0xDDD004, 0);
    icao_filter_add(0xDDD004);
    let score = score_modes_message(&frame, 56);
    assert_eq!(score, 1600, "DF11, CRC=0, IID=0, known ICAO should score 1600");
}

#[test]
fn test_score_df11_good_unknown() {
    let frame = make_df11_frame(0xEEE005, 0);
    let score = score_modes_message(&frame, 56);
    assert_eq!(score, 750, "DF11, CRC=0, IID=0, unknown ICAO should score 750");
}

#[test]
fn test_score_invalid_df() {
    let mut frame = make_df11_frame(0xFFF006, 0);
    frame[0] = 2 << 3;
    let score = score_modes_message(&frame, 56);
    assert_eq!(score, -2, "Invalid DF should score -2");
}

#[test]
fn test_score_unrepairable_crc() {
    let mut frame = make_df17_frame(0x111007, &[0; 7], 5);
    icao_filter_add(0x111007);
    frame[4] ^= 0xFF;
    let score = score_modes_message(&frame, 112);
    assert_eq!(score, -2, "Unrepairable CRC should score -2");
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
fn test_demod_preamble_with_noise_rejected() {
    // Valid preamble followed by noise should not produce messages
    // (proper CRC validation rejects non-Manchester data)
    let preamble = [
        5000, 50, 5000, 50, 50, 50, 50,
        5000, 50, 5000, 50, 50, 50, 50,
        50, 50,
    ];
    let padding = vec![200u16; 40];
    let mut buffer: Vec<u16> = Vec::with_capacity(preamble.len() + 40);
    buffer.extend_from_slice(&preamble);
    buffer.extend_from_slice(&padding);

    let result = demodulate2400(&buffer, buffer.len(), 0);
    assert_eq!(result.len(), 0, "Noise after preamble should not produce valid messages");
}

// ===== score_phase tests =====

use readsb::demod::score_phase;

#[test]
fn test_score_phase_returns_none_for_empty_buffer() {
    let mag: Vec<u16> = vec![];
    let result = score_phase(0, &mag, 0, 112);
    assert!(result.is_none(), "Empty buffer should return None");
}

#[test]
fn test_score_phase_returns_none_for_too_short_buffer() {
    let mag = [0u16; 10];
    let result = score_phase(0, &mag, 0, 112);
    assert!(result.is_none(), "Too-short buffer should return None");
}

// ===== DemodConfig / DemodResult / MagBufStats tests =====

use readsb::demod::{DemodConfig};

#[test]
fn test_demod_result_new_demod_config() {
    let config = DemodConfig::new();
    assert_eq!(config.preamble_threshold, 32768);
}

#[test]
fn test_empty_noise_produces_zero_stats_with_demod_result() {
    let noise: Vec<u16> = vec![100; 1000];
    let result = readsb::demod::demodulate2400_v2(&noise, noise.len(), &DemodConfig::new());
    assert!(result.messages.is_empty(), "No messages from noise");
    assert_eq!(result.stats.noise_low_samples + result.stats.noise_high_samples + result.stats.loud_events, 0,
        "Stats should be zero for uniform noise");
}
