use readsb::demod::{convert_to_magnitude, InputFormat};

fn bytes_per_iq_pair(format: InputFormat) -> usize {
    match format {
        InputFormat::U8 => 2,
        InputFormat::SC16Q11 => 4,
        InputFormat::SC16Q11M => 4,
        InputFormat::F32 => 8,
    }
}

const OVERLAP_SAMPLES: usize = 300;

#[test]
fn test_overlap_assembly_all_formats() {
    for format in &[InputFormat::U8, InputFormat::SC16Q11, InputFormat::F32] {
        let bpiq = bytes_per_iq_pair(*format);
        let overlap_bytes = OVERLAP_SAMPLES * bpiq;
        let data_len = 4096 * bpiq;

        let overlap_tail = vec![0xABu8; overlap_bytes];
        let new_data = vec![0xCDu8; data_len];

        let mut combined = vec![0u8; data_len + overlap_bytes];
        combined[..overlap_bytes].copy_from_slice(&overlap_tail);
        combined[overlap_bytes..overlap_bytes + data_len].copy_from_slice(&new_data[..data_len]);

        assert_eq!(combined.len(), data_len + overlap_bytes);
        assert_eq!(&combined[..overlap_bytes], &vec![0xABu8; overlap_bytes][..]);
        assert_eq!(&combined[overlap_bytes..], &new_data[..]);
    }
}

#[test]
fn test_tail_save_noop_when_n_is_zero() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let overlap_tail = vec![0xABu8; overlap_bytes];
    let n = 0usize;

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, 0);
    assert_eq!(overlap_tail, vec![0xABu8; overlap_bytes]);
}

#[test]
fn test_tail_save_when_n_less_than_overlap() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut overlap_tail = vec![0xABu8; overlap_bytes];
    let short_data = vec![0xFFu8; overlap_bytes / 2];
    let n = short_data.len();

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, overlap_bytes / 2);

    let keep = overlap_bytes - tail_copy;
    overlap_tail.copy_within(tail_copy.., 0);
    overlap_tail[keep..].copy_from_slice(&short_data[n - tail_copy..n]);

    assert_eq!(&overlap_tail[..keep], &vec![0xABu8; keep][..]);
    assert_eq!(&overlap_tail[keep..], &vec![0xFFu8; tail_copy][..]);
}

#[test]
fn test_tail_save_when_n_larger_than_overlap() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut overlap_tail = vec![0xABu8; overlap_bytes];
    let large_data = vec![0xFFu8; overlap_bytes * 3];
    let n = large_data.len();

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, overlap_bytes);

    let keep = overlap_bytes - tail_copy;
    overlap_tail.copy_within(tail_copy.., 0);
    overlap_tail[keep..].copy_from_slice(&large_data[n - tail_copy..n]);

    assert_eq!(overlap_tail, vec![0xFFu8; overlap_bytes]);
}

#[test]
fn test_magnitude_conversion_with_overlap() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let data_bytes = 4096 * bpiq;
    let combined_len = overlap_bytes + data_bytes;

    let combined = vec![128u8; combined_len];
    let mut magnitude = vec![0u16; combined_len / bpiq];

    let count = convert_to_magnitude(&combined, InputFormat::U8, &mut magnitude);

    assert_eq!(count, combined_len / bpiq);
    assert!(magnitude[..count].iter().all(|&v| v == 0));
}

#[test]
fn test_sc16q11_header_skip() {
    let header = vec![0u8; 12];
    let body = vec![0xABu8; 1024];
    let mut input = Vec::with_capacity(header.len() + body.len());
    input.extend_from_slice(&header);
    input.extend_from_slice(&body);

    let stripped = &input[12..];
    assert_eq!(stripped.len(), 1024);
    assert_eq!(stripped, &[0xABu8; 1024]);

    let mut magnitude = vec![0u16; stripped.len() / 4];
    let count = convert_to_magnitude(stripped, InputFormat::SC16Q11, &mut magnitude);
    assert_eq!(count, 256);
}
