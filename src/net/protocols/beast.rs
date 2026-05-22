/// A decoded Beast protocol frame (input side).
pub struct BeastFrame {
    pub timestamp: i64,
    pub frame_type: u8,
    pub payload: Vec<u8>,
    pub rssi: u8,
}

use std::io::{self, Read};

/// Read up to `max_frames` Beast frames from a byte stream.
/// Scans for 0x1a start markers and returns decoded frames.
/// Preserves partial data between reads for fragmented TCP streams.
/// If read times out (WouldBlock/TimedOut) after finding frames, returns the frames.
pub fn read_beast_frames<R: Read>(reader: &mut R, max_frames: usize) -> io::Result<Vec<BeastFrame>> {
    let mut buf = vec![0u8; 65536];
    let mut frames = Vec::new();
    let mut offset = 0usize;

    loop {
        let n = match reader.read(&mut buf[offset..]) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock
                || e.kind() == io::ErrorKind::TimedOut => {
                return Ok(frames);
            }
            Err(e) => return Err(e),
        };
        let total = offset + n;

        let mut i = 0usize;
        let mut consumed = 0usize;
        while i < total && frames.len() < max_frames {
            if buf[i] == 0x1a {
                if let Some(frame) = parse_beast_frame(&buf[i..]) {
                    frames.push(frame);
                    i += 1;
                    while i < total && buf[i] != 0x1a {
                        i += 1;
                    }
                    consumed = i;
                    continue;
                }
            }
            i += 1;
        }
        if consumed < total {
            buf.copy_within(consumed..total, 0);
            offset = total - consumed;
        } else {
            offset = 0;
        }
        if frames.len() >= max_frames {
            break;
        }
    }
    Ok(frames)
}

/// Encode a BeastFrame as a timestamped record for file storage.
/// Format: 6B ts | 1B flags | 1B type | 1B payload_len | payload
pub fn encode_record(frame: &BeastFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(9 + frame.payload.len());
    let ts_bytes = frame.timestamp.to_be_bytes();
    out.extend_from_slice(&ts_bytes[2..]);
    out.push(0x00);
    out.push(frame.frame_type);
    out.push(frame.payload.len() as u8);
    out.extend_from_slice(&frame.payload);
    out
}

/// Decode a timestamped record back into a BeastFrame.
/// RSSI is not stored in records — always returns 0xff.
pub fn decode_record(data: &[u8]) -> Option<BeastFrame> {
    if data.len() < 9 { return None; }
    let ts_bytes = &data[0..6];
    let timestamp = i64::from_be_bytes([0, 0, ts_bytes[0], ts_bytes[1], ts_bytes[2], ts_bytes[3], ts_bytes[4], ts_bytes[5]]);
    let frame_type = data[7];
    let payload_len = data[8] as usize;
    if data.len() < 9 + payload_len { return None; }
    let payload = data[9..9 + payload_len].to_vec();
    Some(BeastFrame { timestamp, frame_type, payload, rssi: 0xff })
}

/// Parse a single Beast frame from a buffer starting at offset 0.
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload (byte-stuffed)>
pub fn parse_beast_frame(data: &[u8]) -> Option<BeastFrame> {
    if data.len() < 9 { return None; }
    if data[0] != 0x1a { return None; }
    if data[1] != 0x32 && data[1] != 0x33 { return None; }

    let ts_bytes = &data[2..8];
    let timestamp = i64::from_be_bytes([0, 0, ts_bytes[0], ts_bytes[1], ts_bytes[2], ts_bytes[3], ts_bytes[4], ts_bytes[5]]);
    let rssi = data[8];
    // Estimate stuffed payload length: 2x expected payload for worst-case stuffing
    let max_stuffed = if data[1] == 0x32 { 14_usize } else { 28_usize };
    let raw_end = (9 + max_stuffed).min(data.len());
    if raw_end <= 9 { return None; }
    let raw_payload = &data[9..raw_end];

    let mut payload = destuff_beast(raw_payload);
    let max_len = if data[1] == 0x32 { 7_usize } else { 14_usize };
    payload.truncate(max_len);
    Some(BeastFrame { timestamp, frame_type: data[1], payload, rssi })
}

/// Protocol-level statistics for a collection of Beast frames.
pub struct BeastAnalysis {
    pub total_frames: usize,
    pub short_frames: usize,
    pub long_frames: usize,
    pub stuffing_errors: usize,
    pub df_counts: [u32; 32],
    pub max_gap_ms: i64,
    pub mean_gap_ms: i64,
    pub gaps_gt_1000ms: usize,
}

/// Compute protocol statistics from a batch of Beast frames.
pub fn beast_analysis(frames: &[BeastFrame]) -> BeastAnalysis {
    let total_frames = frames.len();
    let mut short_frames = 0usize;
    let mut long_frames = 0usize;
    let stuffing_errors = 0usize;
    let mut df_counts = [0u32; 32];

    for frame in frames {
        if frame.frame_type == 0x32 { short_frames += 1; }
        else { long_frames += 1; }
        if !frame.payload.is_empty() {
            let df = (frame.payload[0] >> 3) as usize;
            if df < 32 { df_counts[df] += 1; }
        }
    }

    let mut max_gap_ms = 0i64;
    let mut sum_gaps = 0i64;
    let mut count_gaps = 0;
    for w in frames.windows(2) {
        let gap_ms = (w[1].timestamp - w[0].timestamp) / 1000;
        if gap_ms > max_gap_ms { max_gap_ms = gap_ms; }
        sum_gaps += gap_ms;
        count_gaps += 1;
    }
    let mean_gap_ms = if count_gaps > 0 { sum_gaps / count_gaps as i64 } else { 0 };

    let gaps_gt_1000ms = frames.windows(2).filter(|w| (w[1].timestamp - w[0].timestamp) / 1000 > 1000).count();

    BeastAnalysis { total_frames, short_frames, long_frames, stuffing_errors, df_counts, max_gap_ms, mean_gap_ms, gaps_gt_1000ms }
}

/// Format a BeastAnalysis as a vector of "key\tvalue" lines for the compare report.
pub fn analysis_summary(analysis: &BeastAnalysis) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("Total frames\t{}", analysis.total_frames));
    lines.push(format!("0x32 (short)\t{} ({:.1}%)", analysis.short_frames,
        if analysis.total_frames > 0 { analysis.short_frames as f64 / analysis.total_frames as f64 * 100.0 } else { 0.0 }));
    lines.push(format!("0x33 (long)\t{} ({:.1}%)", analysis.long_frames,
        if analysis.total_frames > 0 { analysis.long_frames as f64 / analysis.total_frames as f64 * 100.0 } else { 0.0 }));
    for (df, &count) in analysis.df_counts.iter().enumerate() {
        if count > 0 {
            lines.push(format!("DF{}\t{}", df, count));
        }
    }
    lines.push(format!("Max gap (ms)\t{}", analysis.max_gap_ms));
    lines.push(format!("Mean gap (ms)\t{}", analysis.mean_gap_ms));
    lines.push(format!("Gaps >1000ms\t{}", analysis.gaps_gt_1000ms));
    lines
}

/// Inverse of escape_beast: collapse 0x1a 0x1a → 0x1a.
pub fn destuff_beast(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        out.push(data[i]);
        if data[i] == 0x1a && i + 1 < data.len() && data[i + 1] == 0x1a {
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

#[allow(dead_code)] // wired from client read_loop; pending Phase 2 Beast framing
pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

/// Escape 0x1a bytes by doubling them (standard Beast byte-stuffing).
fn escape_beast(data: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(data.len());
    for &b in data {
        escaped.push(b);
        if b == 0x1a {
            escaped.push(0x1a);
        }
    }
    escaped
}

/// Encode raw Mode-S frame bytes in standard Beast binary format (0x1a-escaped).
///
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload>
///   - 0x1a: frame start marker (not escaped)
///   - type: 0x32 (short, ≤7B) / 0x33 (long, 14B)
///   - timestamp: 6 bytes big-endian, microseconds (lower 48 bits of `timestamp_us`)
///   - RSSI: signal/256, clamped to 255, 0xff for signal ≤ 0
///   - payload: raw Mode-S bytes with 0x1a byte-stuffing
pub fn encode_beast_output(data: &[u8], signal_level: f64, timestamp_us: i64) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 10);

    out.push(0x1a); // frame start

    let msg_type = if data.len() <= 7 { 0x32 } else { 0x33 };
    out.push(msg_type);

    // Timestamp: 6 bytes big-endian (lower 48 bits of timestamp_us)
    let ts_bytes = timestamp_us.to_be_bytes();
    out.extend_from_slice(&ts_bytes[2..]);

    // RSSI: signal_level/256, 0xff sentinel if no signal
    let rssi = if signal_level <= 0.0 {
        0xff
    } else {
        ((signal_level / 256.0).min(255.0)) as u8
    };
    out.push(rssi);

    // Payload with byte-stuffing
    out.extend_from_slice(&escape_beast(data));

    out
}
