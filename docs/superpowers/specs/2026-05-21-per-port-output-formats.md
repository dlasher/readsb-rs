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

### Cycle 1: Server architecture (per-port channels)

**RED** — Compilation error: `NetworkServer::new()` / `Server::run()` signatures change.

**GREEN** — Three outbound channels, per-parser subscription.

**Tests**: Unit test verifying different parsers get different broadcast receivers.

### Cycle 2: Standard Beast frame structure

**RED** — `test_beast_standard_format`: 14B payload → byte 0 = `0x1a`, byte 1 = `0x33`, bytes 2-7 = zeros, byte 8 = RSSI, byte 9+ = payload.

**GREEN** — Rewrite `encode_beast_output` with standard format and `escape_beast`.

### Cycle 3: Beast byte-stuffing

**RED** — `test_beast_byte_stuffing`: payload with `0x1a` bytes → doubled in output.

**GREEN** — `escape_beast` in the output path.

### Cycle 4: Beast RSSI encoding + sentinel

**RED** — `test_beast_rssi_encoding`: signal=5000 → 19, signal=0 → 0xFF, signal=100000 → 0xFF.

**GREEN** — RSSI mapping formula.

### Cycle 5: Beast frame types

**RED** — `test_beast_frame_types`: 7B → `0x32`, 14B → `0x33`.

**GREEN** — Update type constants.

### Cycle 6: Hex output

**RED** — `test_hex_encode`: `[0x8D, 0x48]` → `"*8D48;\n"`.

**GREEN** — Implement `encode_hex_output`.

### Cycle 7: SBS output — ICAO and signal (mandatory)

**RED** — `test_sbs_encode_icao_signal`: Aircraft with `addr=0x4840D6` → contains `MSG,7,1,1,4840D6` and `MSG,8`.

**GREEN** — Implement `encode_sbs_aircraft` with MSG,7 and MSG,8.

### Cycle 8: SBS — callsign, altitude, position, velocity

**RED** — `test_sbs_encode_callsign`, `test_sbs_encode_altitude`, `test_sbs_encode_position`, `test_sbs_encode_velocity`.

**GREEN** — Extend `encode_sbs_aircraft`.

### Cycle 9: Wire main.rs

**RED** — Compilation: old `message_tx` replaced, `encode_beast_output` signature changed, new encoders missing, periodic SBS task missing.

**GREEN** — Wire all three beast/hex sends in message loop, add SBS task, remove old `encode_beast_output(&DecodedMessage{...})` wrapper.

## Test plan

All new unit tests in `tests/net_compat.rs`. No integration end-to-end tests (no running server).

Existing tests:
- `test_beast_parse_timestamp` — unaffected
- `test_sbs_parse_basic`, `test_hex_parse_basic`, `test_hex_parse_empty` — input parsers, unaffected
- `test_beast_encode_output` — **removed** (replaced by Cycle 2-5 tests)

## Scope boundaries

1. **Inbound Beast parser** unchanged — `client.rs` still expects old MLAT format (`0x10 0x02/0x03`). readsb-rs is primarily an output source, not an MLAT receiver.
2. **Beast timestamp** = zero placeholder. Real 12 MHz counter not implemented.
3. **Hex port naming**: port 30001 is called `net_ri_port` (raw input) in config but also serves hex output. This matches readsb C's convention where the raw input port is bidirectional.
4. **SBS `airground` field** tracked but not yet used to gate MSG,2 (surface position) vs MSG,3 (airborne) — both use MSG,3 for now. Can be refined later.
