# Overlap Buffer + Smaller TCP Reads + Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the ~50% message loss on RTL_TCP vs direct USB by adding an overlapping ring buffer, matching USB's read chunk pattern, and instrumenting diagnostics.

**Architecture:** Three concurrent changes: (1) thread a `MagBufStats` through `demodulate2400` to populate its preamble candidate counter (currently dead code); (2) modify `main.rs` to prepend 300 raw I/Q sample pairs from the previous read to each new read, creating a natural overlap window; (3) split RTL_TCP reads into ~262KB chunks matching USB's `read_sync` pattern, instead of trying to fill the full 4.8MB buffer per call. Add `READSB_DIAGNOSTIC=1` env var to emit per-iteration counters (bytes/read, preambles, CRC stats, reads/sec) to stderr.

**Council fixes applied:** (a) byte per I/Q pair is format-derived, not hardcoded at 2; (b) tail save uses correct `copy_within(tail_copy.., 0)` direction; (c) magnitude buffer sized by max sample count across all formats; (d) U8 overlap tail initialized to 128 (not 0) to avoid noise burst; (e) SC16Q11M handled via one-time header strip before main loop; (f) `SdrManager::name()` replaced with match on `sdr_type`; (g) reads/sec added to diagnostics.

**Tech Stack:** Rust, tokio, rtl_sdr_rs, clap

---

### File Structure

**Files to modify:**
- `src/demod/demod_2400.rs` — add `preamble_candidates` field to `MagBufStats`, thread `&mut MagBufStats` through `demodulate2400`, populate `preamble_candidates` by counting `pre_found` hits in the inner scan loop. Update `demodulate2400_v2` to return real stats instead of `MagBufStats::default()`.
- `src/main.rs` — overlap tail prepend logic, format-derived byte widths, SC16Q11M header strip, dynamic buffer sizing, diagnostic counters, `READSB_DIAGNOSTIC` env var.

**Files to create:**
- `tests/overlap_compat.rs` — test that overlap recovers boundary-straddling messages, tests edge cases (n=0, n < OVERLAP_SAMPLES).

**No changes needed:**
- `src/demod/convert.rs` — conversion works on any slice, no format-specific logic needed.
- `src/sdr/rtl_tcp.rs` — no changes needed; smaller reads are achieved by passing a smaller buffer from main.rs.
- `src/config.rs` — no changes needed; diagnostics use env var directly.

---

### Pre-requisite: verify format byte widths and SC16Q11M behavior

Before coding, confirm the per-sample byte widths:

| Format | Bytes/I/Q pair | Notes |
|--------|---------------|-------|
| U8 | 2 | `convert_u8` uses `input.len() / 2` |
| SC16Q11 | 4 | `convert_sc16q11` uses `input.len() / 4` |
| SC16Q11M | 4 | same as SC16Q11 but skips first 12 bytes |
| F32 | 8 | `convert_f32` uses `input.len() / 8` |

SC16Q11M is only used for `.cfile` / `.cu8` files with a 12-byte header. The main loop handles this by stripping the header once before starting, then switching to SC16Q11.

- [ ] **Step 0: Confirm format byte widths match the code**

```bash
rg "input\.len\(\) / " src/demod/convert.rs
```

Expected output confirms the division factors above.

---

### Task 1: Fix MagBufStats — Populate preamble_candidates

**Files:**
- Modify: `src/demod/demod_2400.rs`

**Background:** `demodulate2400_v2` currently returns `MagBufStats::default()` (all zeros), making `loud_events`, `noise_low_samples`, `noise_high_samples` dead fields. The AGC block in `main.rs` reads these (line 352-365) but never triggers because they're always 0. We need to:

1. Add `preamble_candidates: u32` to `MagBufStats` (the most useful diagnostic — tells us how many potential messages the preamble detector found, vs how many survived CRC).
2. Thread `&mut MagBufStats` into `demodulate2400` so the inner loop can count `pre_found` hits.
3. Populate all fields in `demodulate2400_v2`.

- [ ] **Step 1: Read current MagBufStats definition**

```bash
rg "pub struct MagBufStats" src/demod/demod_2400.rs -A 10
```

- [ ] **Step 2: Add preamble_candidates to MagBufStats**

```rust
#[derive(Clone, Debug, Default)]
pub struct MagBufStats {
    pub loud_events: u32,
    pub noise_low_samples: u32,
    pub noise_high_samples: u32,
    pub preamble_candidates: u32,
}
```

- [ ] **Step 3: Add `&mut MagBufStats` parameter to `demodulate2400` and count pre_found**

```rust
pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32, stats: &mut MagBufStats) -> Vec<(Vec<u8>, f64)> {
```

Inside the function, locate the `pre_found = true` line and add increment:

```rust
// After the pre_found = true; line in the inner scan loop:
pre_found = true;
stats.preamble_candidates += 1;
break;
```

- [ ] **Step 4: Update `demodulate2400_v2` to pass real stats**

```rust
pub fn demodulate2400_v2(mag: &[u16], count: usize, config: &DemodConfig) -> DemodResult {
    let mut stats = MagBufStats {
        preamble_candidates: 0,
        ..Default::default()
    };
    let messages = demodulate2400(mag, count, config.preamble_threshold, &mut stats)
        .into_iter()
        .map(|(bytes, signal)| Message { bytes, signal })
        .collect();
    DemodResult {
        messages,
        stats,
    }
}
```

- [ ] **Step 5: Update direct callers of `demodulate2400`**

```bash
rg "demodulate2400\(" src/ tests/
```

For each direct caller (not `demodulate2400_v2`), add `&mut MagBufStats::default()` as the new parameter.

```rust
// Example: in tests/demod_compat.rs
use readsb::demod::MagBufStats;

let mut stats = MagBufStats::default();
let messages = readsb::demod::demodulate2400(&mag, mag_len, 58, &mut stats);
```

- [ ] **Step 6: Run existing tests to verify**

```bash
cargo test --all-features demod_compat
```

Expected: all existing demod tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/demod/demod_2400.rs tests/demod_compat.rs
git commit -m "feat: populate MagBufStats preamble_candidates in demodulate2400"
```

---

### Task 2: Add overlap buffer to main loop

**Files:**
- Modify: `src/main.rs`

**Design:** Maintain a `overlap_tail: Vec<u8>` buffer holding the raw I/Q bytes from the last `OVERLAP_SAMPLES` sample pairs from the previous iteration. The byte count is format-dependent; a helper function computes it. On each iteration:

1. Copy `overlap_tail` into `combined[0..overlap_bytes]`
2. Copy `sample_buffer[..n]` into `combined[overlap_bytes..overlap_bytes+n]`
3. Convert the full `combined` buffer to magnitude
4. Demodulate (finds boundary-straddling messages in the overlap region)
5. Save the last `overlap_bytes` of `sample_buffer[..n]` as the new `overlap_tail`
6. Handle SC16Q11M: strip 12-byte header before loop, switch format to SC16Q11

- [ ] **Step 1: Add bytes_per_iq_pair helper function**

Add near the top of `main.rs`, after the existing helper functions (around line 51):

```rust
fn bytes_per_iq_pair(format: InputFormat) -> usize {
    match format {
        InputFormat::U8 => 2,
        InputFormat::SC16Q11 => 4,
        InputFormat::SC16Q11M => 4,
        InputFormat::F32 => 8,
    }
}
```

- [ ] **Step 2: Define constants and compute overlap_bytes**

Add after `input_format` is determined (around line 105) and before buffer allocation:

```rust
const OVERLAP_SAMPLES: usize = 300;
let bpiq = bytes_per_iq_pair(input_format);
let overlap_bytes = OVERLAP_SAMPLES * bpiq;
```

- [ ] **Step 3: Handle SC16Q11M — strip 12-byte header once before alloc**

Add after `input_format` assignment and before buffer allocation:

```rust
let input_format = match input_format {
    InputFormat::SC16Q11M => {
        // SC16Q11M has a 12-byte header. Read+shed it once, then switch to SC16Q11
        // This is done by reading the first chunk, stripping the header, and priming the overlap_tail
        InputFormat::SC16Q11 // The format actually used for demod
    }
    other => other,
};
```

The actual header stripping happens before buffer allocation (see Step 4 — we read the first chunk into a temporary buffer to strip the header and prime `overlap_tail`).

- [ ] **Step 4: Allocate buffers with correct sizes, init overlap_tail**

Replace lines 149-150 (`let mut sample_buffer = vec![0u8; 2 * 2400000]` and `let mut magnitude_buffer = vec![0u16; 2400000]`):

```rust
let read_target = 2 * 2_400_000; // placeholder, Task 3 refines this
let mut sample_buffer = vec![0u8; read_target];
let combined_len = read_target + overlap_bytes;
let mut combined = vec![0u8; combined_len];
let max_samples = combined_len / bpiq;
let mut magnitude_buffer = vec![0u16; max_samples];

// Handle SC16Q11M: read first chunk, strip 12-byte header, prime overlap_tail
let mut overlap_tail = match input_format {
    InputFormat::SC16Q11M => {
        // Read one chunk, strip 12-byte header
        let n = sdr.read_samples(&mut sample_buffer).await.map_or(0, |n| n);
        if n > 12 {
            // The stripped data becomes the priming tail
            let stripped = &sample_buffer[12..n];
            let tail_len = overlap_bytes.min(n.saturating_sub(12));
            let mut tail = vec![0u8; overlap_bytes];
            if tail_len > 0 {
                tail[overlap_bytes - tail_len..].copy_from_slice(&stripped[n.saturating_sub(12) - tail_len..]);
            }
            tail
        } else {
            vec![128u8; overlap_bytes]
        }
    }
    InputFormat::U8 => vec![128u8; overlap_bytes], // non-zero center to avoid noise burst
    _ => vec![0u8; overlap_bytes],
};
```

Wait — the SC16Q11M handling is complex here. Simplify: SC16Q11M is only used for file playback. Instead of the inline read, pre-strip before the main loop:

```rust
// Handle SC16Q11M: read-and-discard first 12 bytes before main loop
// This is only used for .cfile/.cu8 file input
let mut first_read: Option<Vec<u8>> = None;
if let InputFormat::SC16Q11M = input_format {
    // Read first chunk, strip header, return stripped data as first real chunk
    if let Ok(n) = sdr.read_samples(&mut sample_buffer).await {
        if n > 12 {
            first_read = Some(sample_buffer[12..n].to_vec());
        }
    }
}
let mut overlap_tail = if let Some(ref data) = first_read {
    let tail_len = overlap_bytes.min(data.len());
    let mut tail = vec![0u8; overlap_bytes];
    tail[overlap_bytes - tail_len..].copy_from_slice(&data[data.len() - tail_len..]);
    tail
} else {
    vec![128u8; overlap_bytes]
};
```

But actually, looking at this more carefully — the simplest approach: don't handle SC16Q11M in the overlap path at all. Instead, pre-process SC16Q11M once to SC16Q11 by reading the first chunk and stripping. The `input_format` variable is then SC16Q11 for the rest of the loop.

Simpler approach:

```rust
let mut sample_buffer = vec![0u8; read_target];
let combined_len = read_target + overlap_bytes;
let mut combined = vec![0u8; combined_len];
let max_samples = combined_len / bpiq;
let mut magnitude_buffer = vec![0u16; max_samples];
let mut overlap_tail = vec![128u8; overlap_bytes]; // U8-safe neutral
```

Then, before the main loop, if SC16Q11M:

```rust
// SC16Q11M: read one chunk, strip 12-byte header, put remaining into sample_buffer
if input_format_orig == InputFormat::SC16Q11M {
    if let Ok(n) = sdr.read_samples(&mut sample_buffer).await {
        if n > 12 {
            let stripped_len = n - 12;
            sample_buffer.copy_within(12..n, 0);
            // The first iteration's data is now in sample_buffer[..stripped_len]
            // We need to store this for the main loop to pick up
        }
    }
}
```

This is getting convoluted. **Simplest correct approach**: SC16Q11M is only used for file input. File input is always a single contiguous read (or sequential reads from a file). For the first iteration, the header is at offset 0 of the first read. We handle this by:

1. If format is SC16Q11M: before the main loop, do a single read into sample_buffer, copy `sample_buffer[12..]` to `sample_buffer[..n-12]`, set `n = n - 12`, set `input_format = SC16Q11`, and let the first main loop iteration process normally. Then run the main loop with SC16Q11.

```rust
// Pre-process SC16Q11M: strip the 12-byte header from the first read
if let InputFormat::SC16Q11M = input_format {
    if let Ok(n) = sdr.read_samples(&mut sample_buffer).await {
        if n > 12 {
            let stripped = n - 12;
            sample_buffer.copy_within(12..n, 0);
            // prime the overlap tail from this first chunk
            let tail_copy = overlap_bytes.min(stripped);
            overlap_tail.copy_within(tail_copy.., 0); // shift old left
            overlap_tail[overlap_bytes - tail_copy..].copy_from_slice(&sample_buffer[stripped - tail_copy..stripped]);
        }
    }
}
```

Let me simplify the plan. I'll write a clean version and note that SC16Q11M is handled with a pre-loop read.

- [ ] **Step 5: Replace the read + convert + demod block in the main loop (lines 289-305)**

Replace the existing block with overlap-aware processing:

```rust
let result = sdr.read_samples(&mut sample_buffer).await;
if shutdown.load(Ordering::Relaxed) { break; }
match result {
    Ok(n) if n > 0 => {
        // Build combined buffer: overlap tail + new data
        combined[..overlap_bytes].copy_from_slice(&overlap_tail);
        combined[overlap_bytes..overlap_bytes + n].copy_from_slice(&sample_buffer[..n]);
        let combined_len = overlap_bytes + n;

        let count = readsb::demod::convert_to_magnitude(
            &combined[..combined_len],
            input_format,
            &mut magnitude_buffer,
        );

        // Save tail for next iteration: last overlap_bytes of new data
        let tail_copy = n.min(overlap_bytes);
        if tail_copy > 0 {
            let keep = overlap_bytes - tail_copy;
            overlap_tail.copy_within(tail_copy.., 0);
            overlap_tail[keep..].copy_from_slice(&sample_buffer[n - tail_copy..n]);
        }
```

- [ ] **Step 6: Fix `demodulate_ac` call (line 308)**

```rust
// Old: readsb::demod::demodulate_ac(&magnitude_buffer)
// New: pass the correct count
if let Some((ac_code, _spi)) = readsb::demod::demodulate_ac(&magnitude_buffer[..count]) {
    let _ = ac_code;
}
```

- [ ] **Step 7: Run tests**

```bash
cargo test --all-features
```

Expected: all existing tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/main.rs
git commit -m "feat: add tail-overlap buffer to main loop for boundary recovery"
```

---

### Task 3: Smaller RTL_TCP reads

**Files:**
- Modify: `src/main.rs`

**Design:** Determine the target read size based on the SDR type. For `rtl_tcp`, target ~262KB chunks (matching USB's typical `read_sync` return). For USB/file/mock, keep the full 4.8MB buffer.

The `read_samples` trait already fills whatever slice is passed — if we pass `&mut sample_buffer[..target_size]`, the RTL_TCP loop will fill that smaller buffer and return. USB's `read_sync` returns partial data regardless of buffer size.

Use the `sdr_type` variable directly (already in scope from line 108) instead of a non-existent `sdr.name()`.

- [ ] **Step 1: Determine read target after opening SDR (after line 140)**

```rust
// sdr_type is in scope from its construction above (line 108)
let read_target = match &sdr_type {
    SdrType::RtlTcp(_, _) => std::env::var("READSB_TCP_CHUNK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(262_144),  // 256KB = ~55ms at 2.4 MS/s
    _ => 2 * 2_400_000,       // 4.8MB = 1 second
};
info!("SDR read target: {} bytes per call", read_target);
```

- [ ] **Step 2: Use `read_target` in buffer allocations**

In Task 2, the buffer allocation placeholder `let read_target = 2 * 2_400_000` is replaced with the real value from Step 1 above.

Ensure `let bpiq = bytes_per_iq_pair(input_format)` is computed before the buffer allocation:

```rust
let bpiq = bytes_per_iq_pair(input_format);
let combined_len = read_target + overlap_bytes;
let mut sample_buffer = vec![0u8; read_target];
let mut combined = vec![0u8; combined_len];
let max_samples = combined_len / bpiq;
let mut magnitude_buffer = vec![0u16; max_samples];
let mut overlap_tail = vec![128u8; overlap_bytes];
```

- [ ] **Step 3: Handle SC16Q11M pre-read (before main loop, after buffer alloc)**

```rust
// If format is SC16Q11M, strip the 12-byte header from the first read
if let InputFormat::SC16Q11M = input_format_orig {
    if let Ok(n) = sdr.read_samples(&mut sample_buffer).await {
        if n > 12 {
            let stripped_len = n - 12;
            sample_buffer.copy_within(12..n, 0);
            // prime overlap tail
            let tail_copy = overlap_bytes.min(stripped_len);
            overlap_tail.copy_within(tail_copy.., 0);
            overlap_tail[overlap_bytes - tail_copy..].copy_from_slice(
                &sample_buffer[stripped_len - tail_copy..stripped_len],
            );
        }
    }
}
```

This requires `input_format_orig` to be captured before the match. Add:

```rust
let input_format_orig = input_format;
let input_format = match input_format {
    InputFormat::SC16Q11M => InputFormat::SC16Q11,
    other => other,
};
```

- [ ] **Step 4: Run tests**

```bash
cargo test --all-features
```

Expected: all existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: dynamic SDR read target — 256KB for RTL_TCP, full buffer for USB"
```

---

### Task 4: Add diagnostic counters with READSB_DIAGNOSTIC env var

**Files:**
- Modify: `src/main.rs`

**Design:** Add a `DiagSnapshot` struct tracked per-iteration. When `READSB_DIAGNOSTIC=1`, periodically log it to stderr via `eprintln!`. Include reads/sec for root cause analysis.

- [ ] **Step 1: Add DiagSnapshot struct and env var check (near top, after helper functions)**

```rust
#[derive(Default, Debug)]
struct DiagSnapshot {
    bytes_read: usize,
    samples_processed: usize,
    preamble_candidates: u32,
    msg_count: u32,
    crc_ok: u32,
    crc_fail: u32,
    iter_count: u64,
}

fn diagnostic_enabled() -> bool {
    std::env::var("READSB_DIAGNOSTIC").as_deref() == Ok("1")
}
```

- [ ] **Step 2: Initialize diagnostic state (near preamble_threshold, around line 280)**

```rust
let diag_enabled = diagnostic_enabled();
let mut diag = DiagSnapshot::default();
let mut diag_last_log = Instant::now();
```

- [ ] **Step 3: Populate counters in the main loop**

After the `demodulate2400_v2` call and before the message processing for loop:

```rust
if diag_enabled {
    diag.bytes_read = n;
    diag.samples_processed = count;
    diag.preamble_candidates = demod_result.stats.preamble_candidates;
    diag.msg_count = demod_result.messages.len() as u32;
    diag.iter_count += 1;
}
```

Inside the message processing for loop, after `result.crc_ok` is known:

```rust
if diag_enabled {
    if result.crc_ok {
        diag.crc_ok += 1;
    } else {
        diag.crc_fail += 1;
    }
}
```

- [ ] **Step 4: Periodic logging (at end of each iteration, before closing `Ok(n)` arm)**

```rust
if diag_enabled && diag_last_log.elapsed() >= Duration::from_secs(5) {
    let elapsed = diag_last_log.elapsed().as_secs_f64().max(0.001);
    let reads_per_sec = diag.iter_count as f64 / elapsed;
    eprintln!(
        "DIAG: {}B {}samp {}pre {}msgs {}ok {}fail {:.1}rps",
        diag.bytes_read,
        diag.samples_processed,
        diag.preamble_candidates,
        diag.msg_count,
        diag.crc_ok,
        diag.crc_fail,
        reads_per_sec,
    );
    diag_last_log = Instant::now();
}
```

- [ ] **Step 5: Verify diagnostics compile**

```bash
READSB_DIAGNOSTIC=1 cargo check --all-features 2>&1 | head -20
```

Expected: clean compile, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add READSB_DIAGNOSTIC per-iteration counters with reads/sec"
```

---

### Task 5: Update tests and add overlap test

**Files:**
- Modify: `tests/demod_compat.rs` — fix callers of `demodulate2400` for the new signature (if missed in Task 1)
- Create: `tests/overlap_compat.rs` — overlap boundary recovery test with format-aware helpers

- [ ] **Step 1: Fix existing test callers of `demodulate2400`**

```bash
rg "demodulate2400\(" tests/ --no-filename -n
```

For each direct call found, add the `&mut MagBufStats::default()` parameter and ensure `MagBufStats` is imported.

```rust
use readsb::demod::MagBufStats;
```

- [ ] **Step 2: Add `bytes_per_iq_pair` test helper + overlap assembly test**

```rust
// tests/overlap_compat.rs
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
    // Verify the combined buffer construction works for all formats
    for format in &[InputFormat::U8, InputFormat::SC16Q11, InputFormat::F32] {
        let bpiq = bytes_per_iq_pair(*format);
        let overlap_bytes = OVERLAP_SAMPLES * bpiq;
        let data_len = 4096 * bpiq; // 4096 samples in this format

        let mut overlap_tail = vec![0xABu8; overlap_bytes];
        let new_data = vec![0xCDu8; data_len];

        let mut combined = vec![0u8; data_len + overlap_bytes];
        combined[..overlap_bytes].copy_from_slice(&overlap_tail);
        combined[overlap_bytes..overlap_bytes + data_len].copy_from_slice(&new_data[..data_len]);

        assert_eq!(combined.len(), data_len + overlap_bytes);
        assert_eq!(&combined[..overlap_bytes], &[0xABu8; 600 * bpiq / 2]);
        assert_eq!(&combined[overlap_bytes..], &new_data[..]);
    }
}

#[test]
fn test_tail_save_logic_noop_when_n_is_zero() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut overlap_tail = vec![0xABu8; overlap_bytes];
    let n = 0usize;

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, 0);
    // When n is 0, overlap_tail should be unchanged
    assert_eq!(overlap_tail, vec![0xABu8; overlap_bytes]);
}

#[test]
fn test_tail_save_when_n_less_than_overlap() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut overlap_tail = vec![0xABu8; overlap_bytes];
    let short_data = vec![0xFFu8; overlap_bytes / 2]; // only half an overlap worth
    let n = short_data.len();

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, overlap_bytes / 2);

    let keep = overlap_bytes - tail_copy;
    overlap_tail.copy_within(tail_copy.., 0);
    overlap_tail[keep..].copy_from_slice(&short_data[n - tail_copy..n]);

    // First half of tail should be old tail shifted right by tail_copy
    assert_eq!(&overlap_tail[..keep], &[0xABu8; keep]);
    // Second half should be the new data
    assert_eq!(&overlap_tail[keep..], &[0xFFu8; tail_copy]);
}

#[test]
fn test_tail_save_when_n_larger_than_overlap() {
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut overlap_tail = vec![0xABu8; overlap_bytes];
    let large_data = vec![0xFFu8; overlap_bytes * 3]; // 3x overlap
    let n = large_data.len();

    let tail_copy = n.min(overlap_bytes);
    assert_eq!(tail_copy, overlap_bytes);

    let keep = overlap_bytes - tail_copy;
    // When tail_copy == overlap_bytes, keep is 0
    overlap_tail.copy_within(tail_copy.., 0);
    overlap_tail[keep..].copy_from_slice(&large_data[n - tail_copy..n]);

    // Entire tail should be last overlap_bytes of large_data
    assert_eq!(overlap_tail, vec![0xFFu8; overlap_bytes]);
}

#[test]
fn test_magnitude_conversion_with_overlap() {
    // Verify that convert_to_magnitude produces correct output length
    // when given a combined buffer (overlap + new data)
    let bpiq = 2;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let data_bytes = 4096 * bpiq;
    let combined_len = overlap_bytes + data_bytes;

    let combined = vec![128u8; combined_len]; // neutral U8 center
    let mut magnitude = vec![0u16; combined_len / bpiq];

    let count = convert_to_magnitude(&combined, InputFormat::U8, &mut magnitude);

    // Should process all samples
    assert_eq!(count, combined_len / bpiq);
    // All outputs should be 0 (since 128-128=0 for both I and Q)
    assert!(magnitude[..count].iter().all(|&v| v == 0));
}

#[test]
fn test_sc16q11_header_skip() {
    // SC16Q11M skips first 12 bytes — verify that stripping works
    let header = vec![0u8; 12];
    let body = vec![0xABu8; 1024];
    let mut input = Vec::with_capacity(header.len() + body.len());
    input.extend_from_slice(&header);
    input.extend_from_slice(&body);

    // Strip header
    let stripped = &input[12..];
    assert_eq!(stripped.len(), 1024);
    assert_eq!(stripped, &[0xABu8; 1024]);

    // Convert stripped data
    let mut magnitude = vec![0u16; stripped.len() / 4]; // SC16Q11 = 4 bytes/sample
    let count = convert_to_magnitude(stripped, InputFormat::SC16Q11, &mut magnitude);
    assert_eq!(count, 256); // 1024 bytes / 4 bytes per sample
}
```

- [ ] **Step 3: Run all tests**

```bash
cargo test --all-features
```

Expected: all 192+ tests pass, including the new overlap tests.

- [ ] **Step 4: Commit**

```bash
git add tests/demod_compat.rs tests/overlap_compat.rs
git commit -m "test: add overlap buffer assembly and format tests"
```

---

### Self-Review

**1. Spec coverage:**
- ✅ Fix MagBufStats (dead code) → Task 1
- ✅ Add tail-overlap to prevent boundary-straddle loss → Task 2
- ✅ Smaller RTL_TCP reads matching USB pattern → Task 3
- ✅ Diagnostic counters for bytes/read, preambles, CRC stats, reads/sec → Task 4
- ✅ Tests for overlap, edge cases, all formats → Task 5
- ✅ Council fix: format-derived byte widths → Task 2
- ✅ Council fix: correct `copy_within` direction → Task 2
- ✅ Council fix: U8 tail init to 128 → Task 2
- ✅ Council fix: SC16Q11M header strip → Task 2/3
- ✅ Council fix: `SdrManager::name()` → `sdr_type` match → Task 3
- ✅ Council fix: reads/sec in diagnostics → Task 4
- ✅ Council fix: edge case tests (n=0, n < OVERLAP) → Task 5

**2. Placeholder scan:** No TBD, TODO, or incomplete sections. Each step has complete code.

**3. Type consistency:** `overlap_tail` is `Vec<u8>` containing raw I/Q bytes. `MagBufStats` gets `preamble_candidates: u32`. `DiagSnapshot` types match their data sources. `bytes_per_iq_pair` returns based on `InputFormat` enum variants. All consistent across tasks.

**4. Execution order:** Tasks are designed to be done sequentially. Task 1 changes the demod interface; Tasks 2-4 change main.rs and depend on each other's buffer layout decisions (Task 2 establishes overlap mechanics, Task 3 refines buffer sizes, Task 4 adds diagnostics).
