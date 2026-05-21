# Per-Port Output Formats Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development (recommended) or executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace single-channel MLAT Beast output with per-format Beast/Hex/SBS outputs on the correct wire formats, matching readsb C behavior.

**Architecture:** Three independent broadcast channels (`beast_tx`, `hex_tx`, `sbs_tx`) feed pre-encoded data to per-port clients. Beast and Hex are per-message (every CRC-OK). SBS is periodic (every 1s, iterating tracked aircraft).

**Tech Stack:** Rust, tokio broadcast channels, timestamp formatting via chrono (or std::time manual math).

---

## File Map

| File | Change |
|------|--------|
| `src/net/protocols/beast.rs` | Rewrite `encode_beast_output` (new sig `(data: &[u8], signal: f64) -> Vec<u8>`); add `escape_beast` helper |
| `src/net/protocols/hex.rs` | Add `encode_hex_output(data: &[u8]) -> Vec<u8>` |
| `src/net/protocols/sbs.rs` | Add `sbs_timestamp(now_ms: i64) -> (String, String)` and `encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8>` |
| `src/net/server.rs` | Replace `message_tx: broadcast::Sender<DecodedMessage>` with `beast_tx`, `hex_tx`, `sbs_tx` (all `Vec<u8>`); update `new()` to return 4-tuple; update `run()` for per-parser subscriptions; remove `with_channel` |
| `src/net/client.rs` | `ClientConnection::handle` accepts `broadcast::Receiver<Vec<u8>>` instead of `broadcast::Receiver<DecodedMessage>` |
| `src/main.rs` | Wire three sends in message loop; add periodic SBS task; remove old `DecodedMessage` Beast wrapper |
| `tests/net_compat.rs` | 11 new tests, remove old `test_beast_encode_output` |
| `src/tracking/validity.rs` | Already has `pub const TRACK_STALE: i64 = 60_000` — no change, but will be imported by sbs.rs |

---

### Task 1: Beast output — standard frame structure

**Files:**
- Modify: `src/net/protocols/beast.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_beast_standard_format`**

Append to `tests/net_compat.rs`:

```rust
#[test]
fn test_beast_standard_format() {
    use readsb::net::protocols::beast;
    let data = [
        0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71,
        0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98,
    ];
    let encoded = beast::encode_beast_output(&data, 0.0);

    assert_eq!(encoded[0], 0x1a, "First byte must be 0x1a (frame start)");
    assert_eq!(encoded[1], 0x33, "14B payload → type 0x33 (long frame)");
    for i in 2..8 {
        assert_eq!(encoded[i], 0x00, "Timestamp bytes {i} must be zero");
    }
    assert_eq!(encoded[8], 0xff, "RSSI byte must be 0xff (signal=0 sentinel)");
    assert_eq!(&encoded[9..23], &data, "Payload must be verbatim at bytes 9-22");
    assert_eq!(encoded.len(), 23, "Frame length must be 1+1+6+1+14 = 23");
}
```

Also add the import at the top of the test file for the new signature:
```rust
// No import change needed — `beast` module is already imported on line 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_beast_standard_format -- --exact`
Expected: FAIL — current `encode_beast_output` takes `&DecodedMessage` not `(&[u8], f64)`, compilation error.

- [ ] **Step 3: Write minimal implementation in `src/net/protocols/beast.rs`**

Replace the entire file:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

/// Encode raw Mode-S frame bytes in standard Beast binary format (0x1a-escaped).
///
/// Format: 0x1a <type> <6B timestamp> <1B RSSI> <payload>
///   - 0x1a: frame start marker (not escaped)
///   - type: 0x32 (short, ≤7B) / 0x33 (long, 14B)
///   - timestamp: 6 bytes big-endian, zeros (placeholder)
///   - RSSI: signal/256, clamped to 255, 0xff for signal ≤ 0
///   - payload: raw Mode-S bytes with 0x1a byte-stuffing
pub fn encode_beast_output(data: &[u8], signal_level: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 10);

    out.push(0x1a); // frame start

    let msg_type = if data.len() <= 7 { 0x32 } else { 0x33 };
    out.push(msg_type);

    // Timestamp: 6 zero bytes (placeholder)
    out.extend_from_slice(&[0u8; 6]);

    // RSSI: hardcoded 0xff for now (mapping added in Task 4)
    out.push(0xff);

    // Payload (byte-stuffing added in Task 2)
    out.extend_from_slice(data);

    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_beast_standard_format -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/net/protocols/beast.rs tests/net_compat.rs
git commit -m "feat: standard Beast frame structure in encode_beast_output"
```

---

### Task 2: Beast byte-stuffing

**Files:**
- Modify: `src/net/protocols/beast.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_beast_byte_stuffing`**

```rust
#[test]
fn test_beast_byte_stuffing() {
    use readsb::net::protocols::beast;
    // Payload with 0x1a at indices 1, 3, 6
    let data = [0x00, 0x1a, 0x84, 0x1a, 0xc3, 0xb3, 0x1d];
    let encoded = beast::encode_beast_output(&data, 0.0);

    // Frame: 0x1a + 0x32 + 6B timestamp + 0xff RSSI + payload(stuffed) = 10 + 9 = 19
    assert_eq!(encoded.len(), 19, "Expected length 10 header + 9 payload bytes (7 raw + 2 stuffed)");
    assert_eq!(encoded[10], 0x00);
    assert_eq!(encoded[11], 0x1a);
    assert_eq!(encoded[12], 0x1a); // stuffed copy
    assert_eq!(encoded[13], 0x84);
    assert_eq!(encoded[14], 0x1a);
    assert_eq!(encoded[15], 0x1a); // stuffed copy
    assert_eq!(encoded[16], 0xc3);
    assert_eq!(encoded[17], 0xb3);
    assert_eq!(encoded[18], 0x1d);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_beast_byte_stuffing -- --exact`
Expected: FAIL — `encoded.len()` is 17 (no stuffing) not 19.

- [ ] **Step 3: Add the `escape_beast` helper and update encoder**

In `src/net/protocols/beast.rs`, before `encode_beast_output`:

```rust
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
```

In `encode_beast_output`, replace:
```rust
    out.extend_from_slice(data);
```
with:
```rust
    out.extend_from_slice(&escape_beast(data));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_beast_byte_stuffing -- --exact`
Expected: PASS

- [ ] **Step 5: Run all beast tests to ensure no regressions**

Run: `cargo test test_beast -- --test-threads=1`
Expected: All existing beast + new tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocols/beast.rs tests/net_compat.rs
git commit -m "feat: add Beast byte-stuffing for 0x1a payload bytes"
```

---

### Task 3: Beast frame types (post-hoc coverage)

**Files:**
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the verification test `test_beast_frame_types`**

```rust
#[test]
fn test_beast_frame_types() {
    use readsb::net::protocols::beast;

    // 7 bytes → type 0x32 (short)
    let encoded_short = beast::encode_beast_output(&[0u8; 7], 0.0);
    assert_eq!(encoded_short[1], 0x32, "7B payload → type 0x32");

    // 14 bytes → type 0x33 (long)
    let encoded_long = beast::encode_beast_output(&[0u8; 14], 0.0);
    assert_eq!(encoded_long[1], 0x33, "14B payload → type 0x33");
}
```

- [ ] **Step 2: Run test to verify it passes immediately**

Run: `cargo test test_beast_frame_types -- --exact`
Expected: PASS (Cycle 1 already implements the type logic). This is post-hoc coverage, not a RED-GREEN cycle.

- [ ] **Step 3: Commit**

```bash
git add tests/net_compat.rs
git commit -m "test: add Beast frame type coverage (post-hoc)"
```

---

### Task 4: Beast RSSI encoding + sentinel

**Files:**
- Modify: `src/net/protocols/beast.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_beast_rssi_encoding`**

```rust
#[test]
fn test_beast_rssi_encoding() {
    use readsb::net::protocols::beast;
    let payload = [0x1A, 0x2B, 0x3C, 0x4D];

    let with_signal = beast::encode_beast_output(&payload, 5000.0);
    assert_eq!(with_signal[8], 19, "RSSI for signal=5000 should be 19");

    let with_zero = beast::encode_beast_output(&payload, 0.0);
    assert_eq!(with_zero[8], 0xff, "RSSI for signal=0 should be 0xff sentinel");

    let with_negative = beast::encode_beast_output(&payload, -1.0);
    assert_eq!(with_negative[8], 0xff, "RSSI for signal=-1 should be 0xff sentinel");

    let with_moderate = beast::encode_beast_output(&payload, 50000.0);
    assert_eq!(with_moderate[8], 195, "RSSI for signal=50000 should be 195");

    let with_clamped = beast::encode_beast_output(&payload, 100000.0);
    assert_eq!(with_clamped[8], 0xff, "RSSI for signal=100000 should be clamped to 0xff");

    let edge = beast::encode_beast_output(&payload, 65536.0);
    assert_eq!(edge[8], 255, "RSSI for signal=65536 should be 255 (max)");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_beast_rssi_encoding -- --exact`
Expected: FAIL — first assertion expects 19, gets 0xff (hardcoded).

- [ ] **Step 3: Replace hardcoded RSSI with mapping**

In `encode_beast_output`, replace:
```rust
    out.push(0xff);
```
with:
```rust
    // RSSI: signal_level/256, 0xff sentinel if no signal
    let rssi = if signal_level <= 0.0 {
        0xff
    } else {
        ((signal_level / 256.0).min(255.0)) as u8
    };
    out.push(rssi);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_beast_rssi_encoding -- --exact`
Expected: PASS

- [ ] **Step 5: Run all beast tests**

Run: `cargo test test_beast -- --test-threads=1`
Expected: All four Beast tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocols/beast.rs tests/net_compat.rs
git commit -m "feat: add RSSI mapping and sentinel to Beast encoder"
```

---

### Task 5: Hex output encoder

**Files:**
- Modify: `src/net/protocols/hex.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_hex_encode_output`**

```rust
#[test]
fn test_hex_encode_output() {
    use readsb::net::protocols::hex;

    let encoded = hex::encode_hex_output(&[0x8D, 0x48, 0x40, 0xD6]);
    assert_eq!(encoded, b"*8D4840D6;\n", "4-byte payload");

    let encoded_empty = hex::encode_hex_output(&[]);
    assert_eq!(encoded_empty, b"*;\n", "empty payload");

    let encoded_wide = hex::encode_hex_output(&[0x00, 0xFF, 0x0A]);
    assert_eq!(encoded_wide, b"*00FF0A;\n", "zero, max, newline bytes");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_hex_encode_output -- --exact`
Expected: FAIL — `encode_hex_output` not found (compilation error).

- [ ] **Step 3: Implement `encode_hex_output` in `src/net/protocols/hex.rs`**

Add at the end of the file:

```rust
/// Encode raw Mode-S bytes as AVR-compatible hex line.
/// Format: *<hex string>;\n
pub fn encode_hex_output(data: &[u8]) -> Vec<u8> {
    let hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!("*{};\n", hex).into_bytes()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_hex_encode_output -- --exact`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/net/protocols/hex.rs tests/net_compat.rs
git commit -m "feat: add hex output encoder (AVR hex line format)"
```

---

### Task 6: SBS output — MSG,7 and MSG,8 (mandatory rows)

**Files:**
- Modify: `src/net/protocols/sbs.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_sbs_encode_icao_signal`**

```rust
#[test]
fn test_sbs_encode_icao_signal() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::AddrType;

    let mut aircraft = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1716300000000);

    // now_ms = 2024-05-21T14:00:00.000
    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300000000);
    let output = String::from_utf8(encoded).unwrap();

    // Should start with MSG,7 (ICAO)
    assert!(output.contains("MSG,7,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"),
        "MSG,7 must contain ICAO and timestamp");

    // Should contain MSG,8 (signal)
    assert!(output.contains("MSG,8,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"),
        "MSG,8 must contain ICAO and timestamp");

    // Lines should be \r\n terminated
    assert!(output.ends_with("\r\n"), "Must end with CRLF");

    // Exactly two lines (MSG,7 + MSG,8)
    assert_eq!(output.lines().count(), 2, "Expected exactly 2 MSG lines");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_sbs_encode_icao_signal -- --exact`
Expected: FAIL — `encode_sbs_aircraft` not found (compilation error).

- [ ] **Step 3: Add `sbs_timestamp` and `encode_sbs_aircraft` to `src/net/protocols/sbs.rs`**

Add at the end of the file:

```rust
use crate::tracking::Aircraft;

/// Convert epoch milliseconds to (YYYY/MM/DD, HH:mm:ss.SSS) timestamp strings.
fn sbs_timestamp(now_ms: i64) -> (String, String) {
    let secs = now_ms / 1000;
    let millis = now_ms % 1000;

    // Days since epoch (simplified, no leap seconds)
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // Date from days since 1970-01-01
    let mut y = 1970i64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    let mut d = remaining_days;
    while d >= month_days[m] {
        d -= month_days[m];
        m += 1;
    }

    let date = format!("{:04}/{:02}/{:02}", y, m + 1, d + 1);
    let time = format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis);
    (date, time)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Encode an aircraft's current state as SBS Basestation CSV lines.
///
/// Always emits MSG,7 (ICAO) and MSG,8 (signal).
/// Conditionally emits MSG,5 (altitude), MSG,1 (callsign),
/// MSG,3 (position), MSG,4 (velocity) when data is valid.
pub fn encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8> {
    let icao = format!("{:06X}", a.addr);
    let (date, time) = sbs_timestamp(now_ms);

    let mut lines = Vec::new();

    // MSG,7 — ICAO (always)
    lines.push(format!(
        "MSG,7,1,1,{},1,{},{},{},{},,,,,,,,,,,,,\r\n",
        icao, date, time, date, time
    ));

    // MSG,8 — Signal level (always)
    let signal_db = a.get_signal_db();
    lines.push(format!(
        "MSG,8,1,1,{},1,{},{},{},{},,,,,,,,,,,{:.1},,,,\r\n",
        icao, date, time, date, time, signal_db
    ));

    lines.join("").into_bytes()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_sbs_encode_icao_signal -- --exact`
Expected: PASS

- [ ] **Step 5: Run full test suite to check for regressions**

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocols/sbs.rs tests/net_compat.rs
git commit -m "feat: SBS encoder with MSG,7 (ICAO) and MSG,8 (signal)"
```

---

### Task 7: SBS — altitude (MSG,5)

**Files:**
- Modify: `src/net/protocols/sbs.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_sbs_encode_altitude`**

```rust
#[test]
fn test_sbs_encode_altitude() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use readsb::tracking::validity::TRACK_STALE;

    let mut aircraft = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1716300000000);
    aircraft.baro_alt = 35000;
    aircraft.baro_alt_valid.update(DataSource::ModeAc, 1716300000000);

    // Test 1: altitude valid (30s later, within TRACK_STALE=60s)
    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300030000);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,5,1,1,4840D6,1,2024/05/21,14:00:30.000,2024/05/21,14:00:30.000,,35000,,,,,,,,,,,"),
        "MSG,5 must be emitted with altitude=35000 when valid");

    // Test 2: altitude stale (120s later, beyond TRACK_STALE)
    let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300120000);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,5"), "MSG,5 must NOT be emitted when altitude is stale");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_sbs_encode_altitude -- --exact`
Expected: FAIL — MSG,5 line not found.

- [ ] **Step 3: Add MSG,5 to `encode_sbs_aircraft`**

In `src/net/protocols/sbs.rs`, add to the import block:
```rust
use crate::tracking::validity::TRACK_STALE;
use crate::types::DataSource;
```

In `encode_sbs_aircraft`, after the MSG,7 block, add:
```rust
    // MSG,5 — Altitude (if valid)
    if a.baro_alt_valid.is_valid(now_ms, TRACK_STALE) {
        lines.push(format!(
            "MSG,5,1,1,{},1,{},{},{},{},,{alt},,,,,,,,,,,,,\r\n",
            icao, date, time, date, time,
            alt = a.baro_alt,
        ));
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_sbs_encode_altitude -- --exact`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocols/sbs.rs tests/net_compat.rs
git commit -m "feat: SBS MSG,5 altitude emission with stale check"
```

---

### Task 8: SBS — callsign (MSG,1)

**Files:**
- Modify: `src/net/protocols/sbs.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_sbs_encode_callsign`**

```rust
#[test]
fn test_sbs_encode_callsign() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use readsb::tracking::validity::TRACK_STALE;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.callsign = "BAW123".to_string();
    aircraft.callsign_valid.update(DataSource::Adsb, now_ms);

    // Test 1: callsign valid
    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,1,1,1,A43EA2,1,"), "MSG,1 must contain ICAO A43EA2");
    assert!(output.contains(",BAW123,"), "MSG,1 must contain callsign BAW123");

    // Test 2: callsign stale (beyond TRACK_STALE)
    let stale = now_ms + TRACK_STALE * 2;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,1"), "MSG,1 must NOT be emitted when callsign is stale");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_sbs_encode_callsign -- --exact`
Expected: FAIL — MSG,1 line not found.

- [ ] **Step 3: Add MSG,1 to `encode_sbs_aircraft`**

In `encode_sbs_aircraft`, after the MSG,5 block, add:
```rust
    // MSG,1 — Callsign (if valid)
    if a.callsign_valid.is_valid(now_ms, TRACK_STALE) && !a.callsign.is_empty() {
        lines.push(format!(
            "MSG,1,1,1,{},1,{},{},{},{},,{cs},,,,,,,,,,,,,,,\r\n",
            icao, date, time, date, time,
            cs = a.callsign,
        ));
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_sbs_encode_callsign -- --exact`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/net/protocols/sbs.rs tests/net_compat.rs
git commit -m "feat: SBS MSG,1 callsign emission with stale check"
```

---

### Task 9: SBS — position (MSG,3) and velocity (MSG,4)

**Files:**
- Modify: `src/net/protocols/sbs.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_sbs_encode_position`**

```rust
#[test]
fn test_sbs_encode_position() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use readsb::tracking::validity::TRACK_STALE;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.lat = 51.5;
    aircraft.lon = -0.5;
    aircraft.baro_alt = 35000;
    aircraft.gs = 220.0;
    aircraft.track = 45.0;
    aircraft.baro_rate = 0;
    aircraft.position_valid.update(DataSource::Adsb, now_ms);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,3,1,1,A43EA2,1,"), "MSG,3 must contain ICAO");
    assert!(output.contains(",35000,"), "MSG,3 must contain altitude 35000");
    assert!(output.contains(",220,"), "MSG,3 must contain ground speed 220");
    assert!(output.contains(",45,"), "MSG,3 must contain track 45");
    // Use string contains with different formatting tolerance
    assert!(output.contains("51.5"), "MSG,3 must contain lat 51.5");
    assert!(output.contains("-0.5"), "MSG,3 must contain lon -0.5");

    // Test stale position
    let stale = now_ms + TRACK_STALE * 2;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,3"), "MSG,3 must NOT be emitted when position is stale");
}
```

- [ ] **Step 2: Write the failing test `test_sbs_encode_velocity`**

```rust
#[test]
fn test_sbs_encode_velocity() {
    use readsb::net::protocols::sbs;
    use readsb::tracking::Aircraft;
    use readsb::types::{AddrType, DataSource};
    use readsb::tracking::validity::TRACK_STALE;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default()
        .as_millis() as i64;

    let mut aircraft = Aircraft::new(0xA43EA2, AddrType::AdsbIcao, now_ms);
    aircraft.gs = 220.0;
    aircraft.track = 45.0;
    aircraft.baro_rate = 0;
    aircraft.gs_valid.update(DataSource::Adsb, now_ms);

    let encoded = sbs::encode_sbs_aircraft(&aircraft, now_ms);
    let output = String::from_utf8(encoded).unwrap();
    assert!(output.contains("MSG,4,1,1,A43EA2,1,"), "MSG,4 must contain ICAO");
    assert!(output.contains(",220,"), "MSG,4 must contain ground speed 220");
    assert!(output.contains(",45,"), "MSG,4 must contain track 45");
    assert!(output.contains(",0,"), "MSG,4 must contain vertical rate 0");

    // Test stale velocity
    let stale = now_ms + TRACK_STALE * 2;
    let encoded = sbs::encode_sbs_aircraft(&aircraft, stale);
    let output = String::from_utf8(encoded).unwrap();
    assert!(!output.contains("MSG,4"), "MSG,4 must NOT be emitted when velocity is stale");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test test_sbs_encode_position test_sbs_encode_velocity -- --test-threads=1`
Expected: FAIL — MSG,3/MSG,4 not found.

- [ ] **Step 4: Add MSG,3 and MSG,4 to `encode_sbs_aircraft`**

In `encode_sbs_aircraft`, after the MSG,1 block, add:

```rust
    // MSG,3 — Position (if position valid and lat/lon resolved)
    if a.position_valid.is_valid(now_ms, TRACK_STALE) && (a.lat != 0.0 || a.lon != 0.0) {
        lines.push(format!(
            "MSG,3,1,1,{},1,{},{},{},{},,{alt},,{gs},{track},{lat},{lon},{vrate},,,,,,\r\n",
            icao, date, time, date, time,
            alt = a.baro_alt,
            gs = a.gs,
            track = a.track,
            lat = a.lat,
            lon = a.lon,
            vrate = a.baro_rate,
        ));
    }

    // MSG,4 — Velocity (if ground speed valid)
    if a.gs_valid.is_valid(now_ms, TRACK_STALE) {
        lines.push(format!(
            "MSG,4,1,1,{},1,{},{},{},{},,,,,{gs},{track},,,{vrate},,,,,,,,,\r\n",
            icao, date, time, date, time,
            gs = a.gs,
            track = a.track,
            vrate = a.baro_rate,
        ));
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test test_sbs_encode_position test_sbs_encode_velocity -- --test-threads=1`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/net/protocols/sbs.rs tests/net_compat.rs
git commit -m "feat: SBS MSG,3 position and MSG,4 velocity emission with stale checks"
```

---

### Task 10: Server architecture — per-port channels

**Files:**
- Modify: `src/net/server.rs`
- Modify: `src/net/client.rs`
- Test: `tests/net_compat.rs`

- [ ] **Step 1: Write the failing test `test_network_server_per_port_channels`**

```rust
#[test]
fn test_network_server_per_port_channels() {
    use readsb::net::server::{NetworkServer, InputParser};

    // Create server with one listener per parser type
    let (server, beast_rx, hex_rx, sbs_rx) = NetworkServer::new(&[
        ("0.0.0.0:0", InputParser::Beast),
        ("0.0.0.0:0", InputParser::Hex),
        ("0.0.0.0:0", InputParser::Sbs),
    ]);

    // Verify three distinct receivers exist and are open
    assert!(!beast_rx.is_closed(), "Beast receiver must be open");
    assert!(!hex_rx.is_closed(), "Hex receiver must be open");
    assert!(!sbs_rx.is_closed(), "SBS receiver must be open");

    // Verify they can send/receive through the channels
    // (broadcast::Sender is Sync — server fields are pub)
    assert!(server.beast_tx.send(vec![0x1a, 0x32]).is_ok(), "Beast send must succeed");
    assert!(server.hex_tx.send(b"*8D48;\n".to_vec()).is_ok(), "Hex send must succeed");
    assert!(server.sbs_tx.send(b"MSG,7,...".to_vec()).is_ok(), "SBS send must succeed");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_network_server_per_port_channels -- --exact`
Expected: FAIL — `NetworkServer::new()` returns 2 values, not 4.

- [ ] **Step 3: Rewrite `src/net/server.rs`**

```rust
use std::io;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::client::ClientConnection;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputParser {
    None,
    Beast,
    Hex,
    Sbs,
}

#[derive(Clone, Debug)]
pub struct DecodedMessage {
    pub data: Vec<u8>,
    pub client_id: u64,
}

pub struct NetworkServer {
    bind_addrs: Vec<(String, InputParser)>,
    pub beast_tx: broadcast::Sender<Vec<u8>>,
    pub hex_tx: broadcast::Sender<Vec<u8>>,
    pub sbs_tx: broadcast::Sender<Vec<u8>>,
    pub incoming_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    pub fn new(addrs: &[(&str, InputParser)]) -> (Self, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>) {
        let (beast_tx, beast_rx) = broadcast::channel(1024);
        let (hex_tx, hex_rx) = broadcast::channel(1024);
        let (sbs_tx, sbs_rx) = broadcast::channel(1024);
        let (in_tx, _) = broadcast::channel(1024);
        let bind_addrs = addrs.iter().map(|(a, p)| (a.to_string(), *p)).collect();
        (NetworkServer { bind_addrs, beast_tx, hex_tx, sbs_tx, incoming_tx: in_tx }, beast_rx, hex_rx, sbs_rx)
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut listeners: Vec<(TcpListener, InputParser)> = Vec::new();
        for (addr, parser) in &self.bind_addrs {
            let listener = TcpListener::bind(addr).await?;
            info!("Listening on {} ({:?})", addr, parser);
            listeners.push((listener, *parser));
        }

        loop {
            let (stream, addr, parser) = match listeners.len() {
                0 => return Err(io::Error::new(io::ErrorKind::NotConnected, "no listeners")),
                1 => {
                    let (stream, addr) = listeners[0].0.accept().await?;
                    (stream, addr, listeners[0].1)
                }
                _ => {
                    let accept_futs: Vec<_> = listeners.iter().map(|(l, _)| Box::pin(l.accept())).collect();
                    let (result, idx, _) = futures::future::select_all(accept_futs).await;
                    let (stream, addr) = result?;
                    (stream, addr, listeners[idx].1)
                }
            };

            let in_tx = self.incoming_tx.clone();

            // Subscribe to the appropriate output channel based on parser
            let out_rx: broadcast::Receiver<Vec<u8>> = match parser {
                InputParser::Beast => self.beast_tx.subscribe(),
                InputParser::Hex => self.hex_tx.subscribe(),
                InputParser::Sbs => self.sbs_tx.subscribe(),
                InputParser::None => {
                    // No output needed for None parser
                    continue;
                }
            };

            tokio::spawn(async move {
                warn!("Client connected: {} ({:?})", addr, parser);
                if let Err(e) = ClientConnection::handle(stream, parser, in_tx, out_rx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}
```

- [ ] **Step 4: Update `src/net/client.rs`**

Change the `handle` signature and `write_loop` parameter type:

```rust
pub async fn handle(
    mut stream: TcpStream,
    parser: InputParser,
    incoming_tx: broadcast::Sender<DecodedMessage>,
    mut outgoing_rx: broadcast::Receiver<Vec<u8>>,
) -> io::Result<()> {
```

The `write_loop` already takes `&mut tokio::net::tcp::WriteHalf` and `&mut broadcast::Receiver<Vec<u8>>` — the parameter type is already compatible. No change needed to `write_loop` internals since it already does `tx.write_all(&msg.data).await` where `msg` is `Vec<u8>`.

Remove the `use super::protocols::{sbs, hex};` import if it's no longer needed for the write_loop (the write_loop doesn't use sbs/hex directly).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test test_network_server_per_port_channels -- --exact`
Expected: PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass. Note: the existing test `test_beast_encode_output` will fail because `encode_beast_output` no longer takes `&DecodedMessage`. Remove that test:

Remove the entire `test_beast_encode_output` block from `tests/net_compat.rs`.

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/net/server.rs src/net/client.rs tests/net_compat.rs
git commit -m "refactor: replace single message_tx with per-port Beast/Hex/SBS channels"
```

---

### Task 11: Wire main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Verify RED — compilation errors**

Before making changes, run `cargo build` to confirm the state is broken:
Run: `cargo build`
Expected: errors about `message_tx` not existing on `NetworkServer`, `encode_beast_output` signature mismatch, etc.

- [ ] **Step 2: Update `NetworkServer::new()` return value and channel clones**

Replace:
```rust
    let (mut net_server, _net_rx) = NetworkServer::new(&[
```
with:
```rust
    let (mut net_server, _beast_rx, _hex_rx, _sbs_rx) = NetworkServer::new(&[
```

Replace:
```rust
    let message_tx = net_server.message_tx.clone();
    let incoming_tx = net_server.incoming_tx.clone();
```
with:
```rust
    let beast_tx = net_server.beast_tx.clone();
    let hex_tx = net_server.hex_tx.clone();
    let sbs_tx = net_server.sbs_tx.clone();
    let incoming_tx = net_server.incoming_tx.clone();
```

- [ ] **Step 3: Update Beast encode call in message loop**

Replace:
```rust
                            let beast_data = encode_beast_output(&DecodedMessage {
                                data: raw_msg.clone(),
                                client_id: 0,
                            });
                            let _ = message_tx.send(DecodedMessage {
                                data: beast_data,
                                client_id: 0,
                            });
```
with:
```rust
                            let beast_data = encode_beast_output(raw_msg, *signal);
                            let _ = beast_tx.send(beast_data);
                            let hex_data = readsb::net::protocols::hex::encode_hex_output(raw_msg);
                            let _ = hex_tx.send(hex_data);
```

- [ ] **Step 4: Add periodic SBS task**

After the incoming message processing task (around line 171), add:

```rust
    // SBS output — periodic aircraft iteration (every 1s)
    let tracker_sbs = tracker.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap_or_default()
                .as_millis() as i64;
            for a in tracker_sbs.registry.iter_aircraft() {
                let sbs_data = readsb::net::protocols::sbs::encode_sbs_aircraft(&a, now);
                let _ = sbs_tx.send(sbs_data);
            }
        }
    });
```

- [ ] **Step 5: Remove unused `DecodedMessage` import if no longer needed**

Check if `DecodedMessage` is still used for the incoming_tx path. It likely is (for `incoming_tx.send(DecodedMessage{...})` in the inbound processing). Keep it.

- [ ] **Step 6: Run build to verify it compiles**

Run: `cargo build`
Expected: exit 0, `Compiling readsb v0.6.0` or similar.

- [ ] **Step 7: Run full test suite**

Run: `cargo test --all-features`
Expected: All tests pass.

- [ ] **Step 8: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Zero warnings.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire per-format Beast/Hex/SBS outputs in main pipeline"
```

---

### Task 12: Remove old test and final cleanup

**Files:**
- Modify: `tests/net_compat.rs`

- [ ] **Step 1: Verify old `test_beast_encode_output` is already removed**

Check the test file to ensure the old MLAT format test is gone:
Run: `grep -n "test_beast_encode_output" tests/net_compat.rs`
Expected: no matches (or removed as part of Task 10).

- [ ] **Step 2: Remove the unused `use readsb::net::DecodedMessage` import**

In `tests/net_compat.rs`, remove:
```rust
use readsb::net::DecodedMessage;
```
if present and unused.

- [ ] **Step 3: Final check — full suite**

Run: `cargo test --all-features` && `cargo clippy -- -D warnings`
Expected: All tests pass, zero clippy warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/net_compat.rs
git commit -m "chore: remove old Beast MLAT test, final cleanup"
```

---

## Self-Review Checklist

- **Spec coverage:** Every section in the spec maps to a task. Beast format (Tasks 1-4), Hex (Task 5), SBS (Tasks 6-9), Server architecture (Task 10), Wiring (Task 11), Cleanup (Task 12).
- **Placeholder scan:** No TODOs, TBDs, or hand-wavy steps. Every step has exact code and commands.
- **Type consistency:** `encode_beast_output(&[u8], f64)` in Task 1 matches the call site signature in Tasks 4 and 11. `broadcast::Receiver<Vec<u8>>` in Task 10 matches `write_loop` in Task 10. `TRACK_STALE` imported from `crate::tracking::validity` in Task 7 matches the real constant location. `encode_sbs_aircraft(&Aircraft, i64)` in Task 6 matches usage in Task 11.
- **Missing imports:** `hex::encode_hex_output` is called as `readsb::net::protocols::hex::encode_hex_output` in Task 11 — the module path exists. `sbs::encode_sbs_aircraft` similarly. Verified.
- **Task 10 note:** Task 10 changes `handle()` to take `broadcast::Receiver<Vec<u8>>`. The existing `write_loop` already operates on `Vec<u8>` — no change needed there. But the `incoming_tx` still uses `DecodedMessage` for the read path — that's unchanged.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-21-per-port-output-formats.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
