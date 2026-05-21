# Per-Port Output Formats: Beast, Hex, SBS

## Problem

readsb-rs sends the same pre-encoded Beast binary data on all three network ports (RI=30001, BO=30005, SBS=30003) via a shared broadcast channel. Downstream clients expecting standard Beast format on port 30005 cannot parse the MLAT Beast frames (`0x10 0x02` / `0x10 0x03` wrapped instead of `0x1a`-based). Clients on the hex and SBS ports receive Beast binary data instead of the expected formats.

## Architecture

### Current

```
main.rs → message_tx (broadcast::Sender<DecodedMessage>)
         → server subscribes all clients to same channel
         → write_loop writes msg.data verbatim to every TCP socket
```

All ports receive identical output (Beast binary, incorrect format).

### Proposed

```
Three independent broadcast channels:

main.rs message loop (per CRC-OK):
  beast_tx.send(encode_beast(raw_msg, signal))
  hex_tx.send(encode_hex(raw_msg))

main.rs periodic task (every 1s):
  for a in tracker.registry.iter_aircraft():
    sbs_tx.send(encode_sbs_aircraft(&a, now_ms))

Server:
  Beast port ← beast_rx
  Hex port   ← hex_rx
  SBS port   ← sbs_rx
```

## Beast Output Format (port 30005)

Standard binary Beast protocol (per `beast.md` reference).

### Frame structure

```
0x1a  <type>  <6B timestamp>  <1B RSSI>  <payload>
```

| Field | Bytes | Description |
|-------|-------|-------------|
| Frame start | 1 | `0x1a` — NOT escaped |
| Frame type | 1 | `0x32` short (≤7B payload), `0x33` long (14B) |
| Timestamp | 6 | 12 MHz counter, big-endian. Zero for now (dump1090-compatible). |
| RSSI | 1 | `signal_level / 256`, clamped to 255. `0xff` for signal ≤ 0. |
| Payload | 7 or 14 | Raw Mode-S frame bytes, with byte-stuffing: every `0x1a` → `0x1a 0x1a` |

### Function

```rust
pub fn encode_beast_output(data: &[u8], signal_level: f64) -> Vec<u8>
```

Helper: `fn escape_beast(data: &[u8]) -> Vec<u8>` doubles any `0x1a` byte.

## Hex Output Format (port 30001)

AVR-compatible hex lines. One line per CRC-OK message.

### Frame structure

```
*<hex payload>;\n
```

- `*` start marker
- Upper-case hex string of raw Mode-S bytes
- `;` end marker
- `\n` line terminator

### Example

```
*8D4840D6202CC371C32CE0576098;\n
```

### Function

```rust
pub fn encode_hex_output(data: &[u8]) -> Vec<u8>
```

## SBS Output Format (port 30003)

Basestation CSV format. Generated **periodically (every ~1s)** by iterating over tracked aircraft — matching readsb C behavior. One or more MSG lines per aircraft per tick.

### Per-aircraft rules

| MSG | Condition | Fields emitted |
|-----|-----------|----------------|
| MSG,7 | Always (first line) | ICAO address |
| MSG,5 | `baro_alt_valid.is_valid(now, TRACK_STALE)` | ICAO, altitude |
| MSG,1 | `callsign_valid.is_valid(now, TRACK_STALE)` | ICAO, callsign |
| MSG,3 | `position_valid.is_valid()` + lat/lon non-zero | ICAO, altitude, speed, track, lat, lon, vertical rate |
| MSG,4 | `gs_valid.is_valid(now, TRACK_STALE)` | ICAO, speed, track, vertical rate |
| MSG,8 | Always (last line) | ICAO, signal level |

Validity window: `TRACK_STALE` (60s) — a field's data must have been updated within 60s to be included.

### Line format (22 CSV fields per MSG record)

```
MSG,<type>,1,1,<icao24>,1,<date1>,<time1>,<date2>,<time2>,<callsign>,<alt>,<speed>,<track>,<lat>,<lon>,<vrate>,<squawk>,<alert>,<emerg>,<spi>,<ident>
```

Unused fields are empty (`,,`). All lines `\r\n` terminated.

### Timestamp format

All lines share the same timestamp (`YYYY/MM/DD,HH:mm:ss.SSS`) computed from `now_ms`.

### Function

```rust
pub fn encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8>
```

Returns concatenated `\r\n`-terminated lines. At minimum MSG,7 and MSG,8 are always emitted.

## Signal / RSSI mapping

```
signal ≤ 0.0    → RSSI = 0xFF    (no-data sentinel, dump1090-compatible)
signal  5000    → RSSI = 19      (5000 / 256 = 19.5 → 19)
signal  50000   → RSSI = 195     (50000 / 256 = 195.3 → 195)
signal ≥ 65536  → RSSI = 255     (clamped)
```

Formula: `((signal_level / 256.0).min(255.0)) as u8` when signal > 0.0, else `0xFF`.

For SBS MSG,8: use `a.get_signal_db()` which returns dB-scale signal.

## Server architecture changes

### `NetworkServer` (`src/net/server.rs`)

Replace single `message_tx` with three channels:

```rust
pub struct NetworkServer {
    bind_addrs: Vec<(String, InputParser)>,
    pub beast_tx: broadcast::Sender<Vec<u8>>,
    pub hex_tx: broadcast::Sender<Vec<u8>>,
    pub sbs_tx: broadcast::Sender<Vec<u8>>,
    pub incoming_tx: broadcast::Sender<DecodedMessage>,
}
```

Constructor creates three outbound channels. `run()` subscribes clients to the appropriate channel based on `InputParser`. `ClientConnection::handle` signature changes to accept the correct receiver.

### `ClientConnection` (`src/net/client.rs`)

`write_loop` stays the same (writes `msg.data` verbatim). `handle()` no longer takes a single `broadcast::Receiver<DecodedMessage>` — it takes a `broadcast::Receiver<Vec<u8>>` for the appropriate channel. The `IncomingParser` only affects the read_loop (input parsing).

## File change map

| File | Change |
|------|--------|
| `src/net/server.rs` | Replace `message_tx` with `beast_tx`, `hex_tx`, `sbs_tx`; per-parser subscription in `run()` |
| `src/net/client.rs` | `ClientConnection::handle` accepts format-specific `broadcast::Receiver<Vec<u8>>` |
| `src/net/protocols/beast.rs` | Rewrite `encode_beast_output(data: &[u8], signal: f64) -> Vec<u8>`; add `escape_beast` helper |
| `src/net/protocols/hex.rs` | Add `encode_hex_output(data: &[u8]) -> Vec<u8>` |
| `src/net/protocols/sbs.rs` | Add `encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8>` |
| `src/main.rs` | Three send calls in message loop; import encoders; add periodic SBS task |
| `tests/net_compat.rs` | 4 beast tests, 1 hex test, 1 sbs test |

## TDD cycles

Each cycle: write the test, verify it fails for the expected reason (not a typo), write minimal code
to make it green, verify all tests still pass, then refactor. No implementation code before the test.

### Cycle 1: Standard Beast frame structure (no byte-stuffing yet)

**RED** — `test_beast_standard_format`

14-byte payload `[0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98]`, signal 0.0:

```
assert_eq!(encoded[0], 0x1a);
assert_eq!(encoded[1], 0x33);              // long frame
for i in 2..8 { assert_eq!(encoded[i], 0x00); }  // timestamp zeros
assert_eq!(encoded[8], 0xff);              // RSSI sentinel (signal=0)
assert_eq!(&encoded[9..23], &data);        // payload verbatim
assert_eq!(encoded.len(), 23);             // 1+1+6+1+14 = 23
```

Verify RED: current encoder returns `0x10 0x02` MLAT format — `encoded[0] != 0x1a`, FAIL.

**GREEN** — Rewrite `encode_beast_output(data: &[u8], signal_level: f64) -> Vec<u8>`:
- `0x1a` leading byte
- Type: `0x32` for len ≤ 7, `0x33` for len 14
- 6 zero timestamp bytes
- RSSI hardcoded `0xff` (mapping added in Cycle 4)
- Payload appended verbatim (no stuffing yet — that's Cycle 2)
- Remove old `encode_beast_output(&DecodedMessage)` signature

**REFACTOR** — None yet.

### Cycle 2: Beast byte-stuffing

**RED** — `test_beast_byte_stuffing`

Payload: `[0x00, 0x1a, 0x84, 0x1a, 0xc3, 0xb3, 0x1d]` (7 bytes with two `0x1a`), signal 0.0:

```
// Frame: 0x1a + 0x32 + 6B timestamp + 0xff RSSI + payload (stuffed) = 10 header + 9 = 19
assert_eq!(encoded[0], 0x1a);
assert_eq!(encoded[1], 0x32);              // short frame
assert_eq!(encoded.len(), 19);             // 1+1+6+1+7+2(stuffing) = 19
assert_eq!(encoded[10], 0x00);             // first payload byte
assert_eq!(encoded[11], 0x1a);             // 0x1a in payload
assert_eq!(encoded[12], 0x1a);             // stuffed copy
assert_eq!(encoded[13], 0x84);
assert_eq!(encoded[14], 0x1a);             // second 0x1a in payload
assert_eq!(encoded[15], 0x1a);             // stuffed copy
assert_eq!(encoded[16], 0xc3);
assert_eq!(encoded[17], 0xb3);
assert_eq!(encoded[18], 0x1d);
```

Verify RED: no stuffing → encoded.len() = 17, assertion `encoded.len() == 19` FAIL.

**GREEN** — Add `fn escape_beast(data: &[u8]) -> Vec<u8>` (doubles `0x1a` bytes). Call it on payload before appending.

**REFACTOR** — None.

### Cycle 3: Beast frame types (0x32 / 0x33)

**RED** — `test_beast_frame_types`

```
let encoded_short = beast::encode_beast_output(&[0u8; 7], 0.0);
assert_eq!(encoded_short[1], 0x32);   // 7-byte short → type 0x32

let encoded_long = beast::encode_beast_output(&[0u8; 14], 0.0);
assert_eq!(encoded_long[1], 0x33);    // 14-byte long → type 0x33
```

Verify RED: current `encode_beast_output` already has `0x32`/`0x33` logic from Cycle 1 — passes immediately. This is a **verification test** confirming the type byte, not a new behavior. Write the test, confirm it goes GREEN on first run, no RED phase needed. Document this as a post-hoc coverage addition, not a RED-GREEN cycle.

Move to next cycle.

### Cycle 4: Beast RSSI encoding + sentinel

**RED** — `test_beast_rssi_encoding`

Payload `[0x1A, 0x2B, 0x3C, 0x4D]` (4 bytes):

```
let with_signal = beast::encode_beast_output(&payload, 5000.0);
assert_eq!(with_signal[8], 19);       // 5000 / 256 = 19.53 → 19

let with_zero = beast::encode_beast_output(&payload, 0.0);
assert_eq!(with_zero[8], 0xff);       // sentinel

let with_negative = beast::encode_beast_output(&payload, -1.0);
assert_eq!(with_negative[8], 0xff);   // sentinel

let with_moderate = beast::encode_beast_output(&payload, 50000.0);
assert_eq!(with_moderate[8], 195);    // 50000 / 256 = 195.3 → 195

let with_clamped = beast::encode_beast_output(&payload, 100000.0);
assert_eq!(with_clamped[8], 0xff);    // clamped to 255, but 255 itself is max before clamp

let edge_65536 = beast::encode_beast_output(&payload, 65536.0);
assert_eq!(edge_65536[8], 255);       // 65536 / 256 = 256 → clamp to 255
```

Verify RED: first assertion fails — RSSI hardcoded to `0xff`, expects 19.

**GREEN** — Replace hardcoded `0xff` with:
```rust
let rssi = if signal_level <= 0.0 {
    0xff
} else {
    ((signal_level / 256.0).min(255.0)) as u8
};
```

**REFACTOR** — None.

### Cycle 5: Hex output encoder

**RED** — `test_hex_encode_output`

```
let encoded = hex::encode_hex_output(&[0x8D, 0x48, 0x40, 0xD6]);
assert_eq!(encoded, b"*8D4840D6;\n");

let encoded_empty = hex::encode_hex_output(&[]);
assert_eq!(encoded_empty, b"*;\n");

let encoded_wide = hex::encode_hex_output(&[0x00, 0xFF, 0x0A]);
assert_eq!(encoded_wide, b"*00FF0A;\n");
```

Verify RED: `encode_hex_output` doesn't exist → compilation error. Valid TDD RED.

**GREEN** — Add to `hex.rs`:
```rust
pub fn encode_hex_output(data: &[u8]) -> Vec<u8> {
    let hex: String = data.iter().map(|b| format!("{:02X}", b)).collect();
    format!("*{};\n", hex).into_bytes()
}
```

**REFACTOR** — None.

### Cycle 6: SBS output — MSG,7 and MSG,8 (mandatory rows)

**RED** — `test_sbs_encode_icao_signal`

Create an `Aircraft` with `addr = 0x4840D6`, `signal_next = 0`, `now_ms = 1716300000000` (2024-05-21T14:00:00.000):

```
let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300000000);
let output = String::from_utf8(encoded).unwrap();

// MSG,7: ICAO only
assert!(output.contains("MSG,7,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"));

// MSG,8: ICAO + signal (0.0 dB since no signal history)
assert!(output.contains("MSG,8,1,1,4840D6,1,2024/05/21,14:00:00.000,2024/05/21,14:00:00.000"));

// Lines are \r\n terminated
assert!(output.ends_with("\r\n"));

// Exactly two lines (MSG,7 + MSG,8)
assert_eq!(output.lines().count(), 2);
```

Verify RED: `encode_sbs_aircraft` doesn't exist → compilation error. Valid TDD RED.

**GREEN** — Add to `sbs.rs`:
```rust
use crate::tracking::Aircraft;

fn sbs_timestamp(now_ms: i64) -> (String, String) {
    // Convert milliseconds since epoch to YYYY/MM/DD and HH:mm:ss.SSS
    let secs = now_ms / 1000;
    let millis = now_ms % 1000;
    // ... date/time formatting
    (date_str, time_str)
}

pub fn encode_sbs_aircraft(a: &Aircraft, now_ms: i64) -> Vec<u8> {
    let icao = format!("{:06X}", a.addr);
    let (date, time) = sbs_timestamp(now_ms);

    let mut lines = Vec::new();

    // MSG,7 — ICAO
    lines.push(format!("MSG,7,1,1,{},1,{},{},{},{},,,,,,,,,,,,,\r\n",
        icao, date, time, date, time));

    // MSG,8 — ICAO + signal
    let signal = a.get_signal_db();
    lines.push(format!("MSG,8,1,1,{},1,{},{},{},{},,,,,,,,,,,{:.1},,,,\r\n",
        icao, date, time, date, time, signal));

    lines.join("").into_bytes()
}
```

**REFACTOR** — None.

### Cycle 7: SBS — altitude (MSG,5)

**RED** — `test_sbs_encode_altitude`

Aircraft with `addr = 0x4840D6`, `baro_alt = 35000`, `baro_alt_valid` updated at `t = 1716300000000`, valid window = `now_ms = 1716300030000` (30s later, within TRACK_STALE 60s):

```
let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300030000);
let output = String::from_utf8(encoded).unwrap();

assert!(output.contains("MSG,5,1,1,4840D6,1,2024/05/21,14:00:30.000,2024/05/21,14:00:30.000,,35000,,,,,,,,,,,"));
```

And a test for stale altitude — same update time but now at t + 120s (beyond TRACK_STALE):
```
let encoded = sbs::encode_sbs_aircraft(&aircraft, 1716300120000);
let output = String::from_utf8(encoded).unwrap();
assert!(!output.contains("MSG,5"));  // omitted
```

Verify RED: asserts `output.contains("MSG,5")` — but MSG,5 line not emitted. FAIL.

**GREEN** — In `encode_sbs_aircraft`, after MSG,7, check `baro_alt_valid.is_valid(now_ms, TRACK_STALE)` and emit MSG,5 if valid. Import `TRACK_STALE` from `crate::tracking::validity`.

**REFACTOR** — None.

### Cycle 8: SBS — callsign (MSG,1)

**RED** — `test_sbs_encode_callsign`

Aircraft with `addr = 0xA43EA2`, `callsign = "BAW123"`, `callsign_valid` updated at `t = now_ms`:

```
let output = String::from_utf8(sbs::encode_sbs_aircraft(&aircraft, now_ms)).unwrap();
assert!(output.contains("MSG,1,1,1,A43EA2,1,"));
assert!(output.contains(",BAW123,"));
assert!(output.contains(",,,,,,,,,,,,,,")); // after callsign, fields 11-22 mostly empty
```

Stale callsign test: `now_ms + 120_000` (120s → beyond TRACK_STALE 60s):
```
let output = String::from_utf8(sbs::encode_sbs_aircraft(&aircraft, now_ms + 120_000)).unwrap();
assert!(!output.contains("MSG,1"));
```

Verify RED: MSG,1 missing → FAIL.

**GREEN** — Check `callsign_valid.is_valid(now_ms, TRACK_STALE)` after MSG,7/MSG,5, emit MSG,1.

**REFACTOR** — Consider extracting the timestamp pair (`date`, `time`) to avoid recomputing it in each line builder.

### Cycle 9: SBS — position (MSG,3) and velocity (MSG,4)

**RED** — `test_sbs_encode_position`

Aircraft with `position_valid` updated, `lat = 51.5`, `lon = -0.5`, `baro_alt = 35000`, `gs = 220.0`, `track = 45.0`, `baro_rate = 0`:

```
assert!(output.contains("MSG,3,1,1,A43EA2,1,"));
assert!(output.contains(",35000,"));        // altitude
assert!(output.contains(",220,"));          // ground speed
assert!(output.contains(",45,"));           // track
assert!(output.contains(",51.5,"));         // lat
assert!(output.contains(",-0.5,"));         // lon
```

Stale position (validity expired, lat/lon still non-zero but position_valid stale):
```
assert!(!output.contains("MSG,3"));
```

**RED** — `test_sbs_encode_velocity`

Aircraft with `gs_valid`, `gs = 220.0`, `track = 45.0`, `baro_rate = 0`, no position_valid:

```
assert!(output.contains("MSG,4,1,1,A43EA2,1,"));
assert!(output.contains(",220,"));
assert!(output.contains(",45,"));
assert!(output.contains(",0,"));            // vertical rate
```

Stale velocity:
```
assert!(!output.contains("MSG,4"));
```

Verify RED: MSG,3/MSG,4 checks fail → FAIL.

**GREEN** — After MSG,7/MSG,5/MSG,1:
- `position_valid.is_valid(now_ms, TRACK_STALE)` AND `a.lat != 0.0 || a.lon != 0.0` → MSG,3 with lat/lon/alt/gs/track/vrate
- `gs_valid.is_valid(now_ms, TRACK_STALE)` → MSG,4 with gs/track/vrate

**REFACTOR** — Extract field formatting helpers. The MSG line builder is getting repetitive.

### Cycle 10: Server architecture (per-port channels)

**RED** — `test_network_server_per_port_channels`

```rust
let (server, beast_rx, hex_rx, sbs_rx) = NetworkServer::new(&[
    ("0.0.0.0:0", InputParser::Beast),
    ("0.0.0.0:0", InputParser::Hex),
    ("0.0.0.0:0", InputParser::Sbs),
]);
// Verify three distinct senders/receivers exist
assert!(!beast_rx.is_closed());
assert!(!hex_rx.is_closed());
assert!(!sbs_rx.is_closed());
```

Verify RED: `NetworkServer::new()` doesn't return 4 items (only returns 2) → compilation error.

**GREEN** — Replace `message_tx: broadcast::Sender<DecodedMessage>` with three `Vec<u8>` channels:
```rust
pub struct NetworkServer {
    bind_addrs: Vec<(String, InputParser)>,
    pub beast_tx: broadcast::Sender<Vec<u8>>,
    pub hex_tx: broadcast::Sender<Vec<u8>>,
    pub sbs_tx: broadcast::Sender<Vec<u8>>,
    pub incoming_tx: broadcast::Sender<DecodedMessage>,
}
```

`new()` returns `(Self, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>)`.

`run()` subscribes each client to the channel matching its `InputParser`:
- `InputParser::Beast` → `beast_tx.subscribe()`
- `InputParser::Hex` → `hex_tx.subscribe()`
- `InputParser::Sbs` → `sbs_tx.subscribe()`
- `InputParser::None` → skip (no write_loop)

Remove `with_channel` — no longer used.

**REFACTOR** — `ClientConnection::handle` takes `broadcast::Receiver<Vec<u8>>` instead of `broadcast::Receiver<DecodedMessage>`. `write_loop` doesn't change (already writes `Vec<u8>`).

### Cycle 11: Wire main.rs

**RED** — Compilation errors:
- `net_server.message_tx` no longer exists → `net_server.beast_tx`, `net_server.hex_tx`, `net_server.sbs_tx`
- `NetworkServer::new()` returns 4 values, not 2
- `encode_beast_output(&DecodedMessage{...})` → `encode_beast_output(raw_msg, *signal)`
- `hex::encode_hex_output` not imported
- `sbs::encode_sbs_aircraft` not imported
- No periodic SBS task

**GREEN** —
```rust
let (mut net_server, _beast_rx, _hex_rx, _sbs_rx) = NetworkServer::new(&[...]);
let beast_tx = net_server.beast_tx.clone();
let hex_tx = net_server.hex_tx.clone();
let sbs_tx = net_server.sbs_tx.clone();
let incoming_tx = net_server.incoming_tx.clone();

// In message loop:
let beast_data = encode_beast_output(raw_msg, *signal);
let _ = beast_tx.send(beast_data);
let hex_data = hex::encode_hex_output(raw_msg);
let _ = hex_tx.send(hex_data);

// Periodic SBS task:
let tracker_sbs = tracker.clone();
tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tick.tick().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap_or_default()
            .as_millis() as i64;
        for a in tracker_sbs.registry.iter_aircraft() {
            let sbs_data = sbs::encode_sbs_aircraft(&a, now);
            let _ = sbs_tx.send(sbs_data);
        }
    }
});
```

**REFACTOR** — Remove the old `DecodedMessage` wrapper at the Beast send site. The `message_tx` clone is removed.

### Cycle 12: Verify full test suite

Run `cargo test --all-features` — all existing tests plus new tests must pass. Run `cargo clippy -- -D warnings` — zero warnings. Fix any failures before considering implementation complete.

## Test plan

All new unit tests in `tests/net_compat.rs`. No integration tests (no running TCP server).

| Test | Cycle | What it proves |
|------|-------|---------------|
| `test_beast_standard_format` | 1 | `0x1a` start, type `0x33`, timestamps zeros, RSSI sentinel, payload |
| `test_beast_byte_stuffing` | 2 | `0x1a` doubled in payload |
| `test_beast_frame_types` | 3 | 7B→`0x32`, 14B→`0x33` (post-hoc coverage) |
| `test_beast_rssi_encoding` | 4 | Signal-to-RSSI mapping (zero, negative, normal, clamped) |
| `test_hex_encode_output` | 5 | Hex encoding with `*`, `;`, `\n` |
| `test_sbs_encode_icao_signal` | 6 | MSG,7 (ICAO) + MSG,8 (signal), `\r\n` termination |
| `test_sbs_encode_altitude` | 7 | MSG,5 (altitude valid + stale) |
| `test_sbs_encode_callsign` | 8 | MSG,1 (callsign valid + stale) |
| `test_sbs_encode_position` | 9 | MSG,3 (position valid + stale) |
| `test_sbs_encode_velocity` | 9 | MSG,4 (velocity valid + stale) |
| `test_network_server_per_port_channels` | 10 | Three channels created, distinct per InputParser |

Existing tests:
- `test_beast_parse_timestamp` — unaffected
- `test_sbs_parse_basic`, `test_hex_parse_basic`, `test_hex_parse_empty` — input parsers, unaffected
- `test_beast_encode_output` — **removed** (tests old MLAT format; replaced by cycles 1-4)

## Scope boundaries

1. **Inbound Beast parser** unchanged — `client.rs` still expects old MLAT format (`0x10 0x02/0x03`). readsb-rs is primarily an output source, not an MLAT receiver.
2. **Beast timestamp** = zero placeholder. Real 12 MHz counter not implemented.
3. **Hex port naming**: port 30001 is called `net_ri_port` (raw input) in config but also serves hex output. This matches readsb C's convention where the raw input port is bidirectional.
4. **SBS `airground` field** tracked but not yet used to gate MSG,2 (surface position) vs MSG,3 (airborne) — both use MSG,3 for now. Can be refined later.
