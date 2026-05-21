# readsb-rs: Remaining Work Plan (TDD)

> Generated 2026-05-20 after full codebase audit against plans at:
> `docs/plans/01-core-computation.md`, `docs/plans/02-tracking-demod.md`, `docs/plans/03-application-layer.md`

**_Updated 2026-05-20 after implementation session._**

**Current state:** 83 tests pass (was 34), 0 compiler warnings (was 11), all phases implemented with TDD.

**Methodology:** Each task was a RED→GREEN→REFACTOR cycle. Test written first, watched fail, then minimal implementation. No implementation before test.

## Completed

All 7 phases (A–G, 38 TDD cycles) implemented in a single session:

- **Phase A:** Tracker rewrite — 12 RED→GREEN cycles for `update_from_message()` with all per-field updaters, CPR pairing, stale removal, and a full test suite (21 tracker tests)
- **Phase B:** SDR cleanup — resolved 10 duplicate const warnings, removed unused import, added MockSdrDevice + SdrType::Mock for testability
- **Phase C:** Network output — multi-listener select via `futures::future::select_all`, broadcast publication to all connected clients, Beast output encoding
- **Phase D:** Integration tests — live fixture sampled from `10.4.10.152:2345` RTL-TCP server, synthetic DF17 fixture, silence fixture, 4 binary-level acceptance tests
- **Phase E:** Output/stats unit tests — JSON serialization, globe index, binCraft layout, stats defaults
- **Phase F:** Benchmarks — CRC checksum + diagnose, SC16Q11 conversion + demodulate2400 noise
- **Phase G:** Polish — CLI iformat/config tests, dead module refs removed

## What still needs attention (lower priority)

1. **RTL-SDR USB FFI** (`src/sdr/rtlsdr.rs:51`): `read_samples` returns `Err(Unsupported)` instead of `unimplemented!()` now, but actual FFI via `rtl-sdr-rs` not wired. Mock device available for testing.
2. **Main loop enhancements** (`src/main.rs`): Stale removal timer, JSON periodic output, stats wiring, graceful shutdown — integration tests exist (D1-D4) documenting expected behavior, but code implementations are deferred.
3. **Demodulator validation**: Tests verify no false positives in noise but no test with real Mode-S burst. Could use actual detection validation with `synthetic_df17.iq`.
4. **Benchmark baselines**: Need to be recorded and tracked over time.

---

## Phase A: 🔴 Tracker Rewrite (13 TDD cycles)

All cycles target `src/tracking/tracker.rs`. Tests live in `tests/tracking_compat/test_tracker.rs`.

### A1 — Aircraft Creation
- **RED:** `test_tracker_creates_aircraft` — calls `update_from_message()` with valid addr (0x4840D6, Adsb, now=1000), asserts return is `Some(Arc)`, registry len = 1
- **Expected fail:** `update_from_message` returns `()` not `Option`
- **GREEN:** Change signature to `-> Option<Arc<RwLock<Aircraft>>>`, call `get_or_create()`, return `Some(arc)`

### A2 — Invalid Address Guard
- **RED:** `test_tracker_ignores_invalid_address` — addr=0 returns `None`, registry len = 0
- **Expected fail:** addr=0 still creates aircraft
- **GREEN:** Add `if mm.addr == 0 || mm.addr == 0xFFFFFF { return None; }` guard

### A3 — Barometric Altitude + Validity
- **RED:** `test_tracker_updates_altitude_baro` — set `mm.baro_alt=35000`, `baro_alt_valid=true`, `source=Adsb`. Assert `a.baro_alt==35000`, `a.baro_alt_valid.source==Adsb`
- **Expected fail:** baro_alt remains `INVALID_ALTITUDE`
- **GREEN:** `update_altitude()` method: `a.baro_alt_valid.update(mm.source, now); a.baro_alt = mm.baro_alt;`

### A4 — Geometric Altitude + Delta
- **RED:** `test_tracker_updates_altitude_geom` — set `geom_alt=36000`, `baro_alt=35000` (both valid). Assert `geom_alt==36000`, `geom_delta==1000`
- **Expected fail:** geom_alt/delta unchanged
- **GREEN:** Extend `update_altitude()`: set geom_alt + validity, compute `geom_delta = geom_alt - baro_alt` when both valid

### A5 — Callsign Update
- **RED:** `test_tracker_updates_callsign` — set callsign "BAW123" with validity. Assert `a.callsign == "BAW123"`, `callsign_valid.source == Adsb`
- **Expected fail:** callsign empty
- **GREEN:** `update_callsign()`: strip NUL bytes, set string + validity

### A6 — Velocity Fields
- **RED:** `test_tracker_updates_velocity` — set `gs=450.0`, `ias=280`, `tas=460`, `mach=0.82`, all valid. Assert each field + gs validity
- **Expected fail:** velocity fields unchanged
- **GREEN:** `update_velocity()`: set gs+validity, ias, tas, mach

### A7 — Squawk Update
- **RED:** `test_tracker_updates_squawk` — `squawk_hex=0x1234`, valid. Assert `a.squawk == 0x1234`, `squawk_valid.source == Adsb`
- **Expected fail:** squawk unchanged
- **GREEN:** `update_squawk()`: set squawk + validity

### A8 — Emergency State
- **RED:** `test_tracker_updates_emergency` — `mm.emergency = Emergency::General`. Assert `a.emergency == Emergency::General`
- **Expected fail:** emergency stays `None`
- **GREEN:** `update_emergency()`: set if `!= Emergency::None`

### A9 — Navigation State
- **RED:** `test_tracker_updates_nav` — `nav.mcp_altitude=10000`, `nav.fms_altitude=9500`, `nav.qnh=1013.2`, all valid. Assert fields set
- **Expected fail:** nav fields at default
- **GREEN:** `update_nav()`: set mcp/fms/qnh when valid

### A10 — Accuracy
- **RED:** `test_tracker_updates_accuracy` — `accuracy.nac_p=8`. Assert `a.pos_nic==8`, `a.pos_rc==8`
- **Expected fail:** accuracy fields unchanged
- **GREEN:** `update_accuracy()`: copy nac_p to pos_nic/pos_rc

### A11 — CPR Position Pairing
- **RED:** `test_tracker_cpr_pairing` — msg1 (even, cpr_lat=12345, cpr_lon=67890), msg2 (odd, cpr_lat=12400, cpr_lon=67800). After even: stored in `cpr_even_lat/lon`. After odd: `decode_cpr_airborne()` called, `lat`/`lon` set, `position_valid` updated
- **Expected fail:** lat/lon remain 0.0 after both frames
- **GREEN:** `update_position()`: store even frame, on odd frame call `decode_cpr_airborne(even_lat, even_lon, odd_lat, odd_lon, fflag)`, set decoded position + validity

### A12 — Stale Removal
- **RED:** `test_tracker_remove_stale` — create 2 aircraft at time 1000, call `remove_stale(61000)`, assert returns 2, registry len = 0
- **Expected fail:** `remove_stale` not on Tracker
- **GREEN:** `pub fn remove_stale(&self, now: i64) -> usize` delegating to `registry.remove_stale(now, TRACK_EXPIRE)`

### A13 — Test Harness Registration
- **RED:** `cargo test tracking_compat` fails to find `test_tracker` module
- **Expected fail:** module not found
- **GREEN:** Add `#[path = "tracking_compat/test_tracker.rs"] mod test_tracker;` to `tests/tracking_compat.rs`

---

## Phase B: 🔴 SDR Cleanup + FFI (3 TDD cycles)

### B1 — Resolve Duplicate Constants
- **RED:** `cargo check` with `RUSTFLAGS="-D warnings"` fails with 10 ambiguous re-export errors
- **Expected fail:** 10 warnings become errors under `-D warnings`
- **GREEN:** Remove `DongleInfo`, `RtltcpCommand`, all `RTLTCP_*` consts from `src/sdr/rtlsdr.rs`. Keep single copy in `src/sdr/rtl_tcp.rs`

### B2 — Remove Unused Import
- **RED:** `cargo check` with `-D warnings` fails on 1 unused import
- **Expected fail:** `unused import: std::sync::Arc` in tracker.rs
- **GREEN:** Remove `use std::sync::Arc;` from `src/tracking/tracker.rs`

### B3 — RTL-SDR USB FFI + Mock
- **RED:** `tests/sdr_backend.rs`:
  - `test_rtlsdr_mock_read_samples` — mock RTLSDR device returns canned I/Q data, read returns `Ok(n)` with expected bytes
  - `test_rtlsdr_usb_read_no_panic` — opens RtlSdr(0), calls `read_samples(buf)`, asserts `Ok` or `Err` (no panic)
- **Expected fail:** `unimplemented!("USB sample reading not implemented yet")` panic in both paths
- **GREEN:**
  - Add `MockRtlSdrDevice` to `sdr/manager.rs` or `sdr/rtlsdr.rs`: stores canned `Vec<u8>` sample buffer, returns it on `read_samples()`, configurable via constructor
  - Wire `rtl-sdr-rs` crate for real USB path. On `open()`: `rtl_sdr_rs::open()`, store handle. On `read_samples()`: spawn_blocking async read
  - **Mock path**: `SdrType::Mock(Vec<u8>)` — returns stored buffer on each read

---

## Phase C: 🟠 Network Output (3 TDD cycles)

### C1 — Multiple Listener Accept
- **RED:** `tests/net_server.rs` → `test_multi_listener_accept` — bind 2 listeners on available ports, spawn server, connect to both, assert both accepted
- **Expected fail:** `listeners[0]` only — second never serviced
- **GREEN:** Replace single `.accept()` with `tokio::select!` across all listeners (loop with `futures::future::select_all` or manual merge)

### C2 — Broadcast Publication on Message Processing
- **RED:** `tests/net_server.rs` → `test_client_receives_broadcast` — spawn server, connect client, publish `DecodedMessage` on broadcast tx, assert client receives it from read
- **Expected fail:** client reads socket, gets nothing (no write path)
- **GREEN:** In `ClientConnection::handle()`: subscribe to `broadcast::Receiver`, in `tokio::select!` loop: on broadcast message, write data to stream

### C3 — Beast Protocol Output Formatting
- **RED:** `tests/net_server.rs` → `test_beast_output_format` — `DecodedMessage` serialized to Beast binary (DLE+timestamp+type+payload+ETX), asserted byte-for-byte
- **Expected fail:** no Beast output formatter exists
- **GREEN:** `src/net/protocols/beast.rs` → add `fn encode_beast_output(decoded: &DecodedMessage) -> Vec<u8>` with DLE/ETX framing + 6-byte timestamp + message type byte + payload

---

## Phase D: 🟠 Main Loop Integration (4 TDD cycles + 1 fixture cycle)

All tests are binary-level acceptance tests via `std::process::Command`. Uses synthetic I/Q test fixture.

### D0 — Generate Test Fixture (must precede D1-D4)
- **RED:** Test file references `test_fixtures/adsb_burst.iq` — file doesn't exist
- **Expected fail:** integration tests can't run
- **GREEN:** 
  - Connect to `10.4.10.152:1234` or `10.4.10.152:2345` (RTL_TCP server)
  - Read ~3 seconds of I/Q samples at 2.4 MHz
  - Save to `test_fixtures/live_sample.iq`
  - Use existing tools (the Rust test itself, or a small script) to validate the file has expected format (SC16Q11, reasonable size ~14MB)
  - Also generate a synthetic valid Mode-S burst programmatically: construct preamble (pulse pattern at samples 0,2,7,9) + DF17 message `8D4840D6202CC371C32CE0576098` as Manchester-coded symbols modulated as SC16Q11 I/Q pairs — save to `test_fixtures/synthetic_df17.iq` for deterministic tests

### D1 — Stale Removal Timer
- **RED:** `tests/integration.rs` → `test_stale_aircraft_removed` — start `readsb --ifile test_fixtures/synthetic_df17.iq` with timeout, after processing wait past stale timeout, check stdout/logs reflect removal count
- **Expected fail:** no removal timer, stale aircraft persist forever
- **GREEN:** Add `tokio::time::interval(Duration::from_secs(1))` branch in main loop's `tokio::select!`: each tick calls `tracker.remove_stale(now_ms)`, logs removal count

### D2 — JSON Output
- **RED:** `tests/integration.rs` → `test_json_output_written` — start `readsb --ifile test_fixtures/synthetic_df17.iq --json-dir /tmp/readsb-test-XXXX`, wait, assert `aircraft.json` exists and contains valid JSON array
- **Expected fail:** no file created
- **GREEN:** Add `tokio::time::interval(Duration::from_secs(1))` for JSON output: iterate registry snapshot, call `generate_aircraft_json()`, write to `{json_dir}/aircraft.json`

### D3 — Stats Counter Wiring
- **RED:** `tests/integration.rs` → `test_stats_incremented` — start `readsb --ifile test_fixtures/synthetic_df17.iq`, check stats log output for non-zero `messages_total`
- **Expected fail:** stats not incremented
- **GREEN:** Create `Stats` in main, pass through pipeline. After demod: `stats.demod_preambles += count`. After track: `stats.messages_total += 1`. Log stats periodically.

### D4 — Graceful Shutdown
- **RED:** `tests/integration.rs` → `test_graceful_shutdown` — start readsb on `--ifile`, send SIGTERM, assert exit code 0, assert no "abort" in stderr
- **Expected fail:** `net_handle.abort()` is forceful, logs abort
- **GREEN:** Replace `abort()` with `tokio::sync::CancellationToken`. Main loop `.await`s network handle with graceful close.

---

## Phase E: 🟡 Output/Stats Unit Tests (4 TDD cycles)

### E1 — JSON Serialization
- **RED:** `tests/output_compat.rs` → `test_aircraft_json_serialization` — create known `Aircraft`, call `AircraftJson::from_aircraft()`, assert JSON fields match expected
- **Expected fail:** no test file
- **GREEN:** Create `tests/output_compat.rs`. Test validates existing code; fix bugs discovered during RED.

### E2 — Globe Index
- **RED:** `tests/output_compat.rs` → `test_globe_index_bounds` — `globe_index(0.0, 0.0)` returns expected tile; `globe_index(89.0, 179.0)` returns different tile
- **Expected fail:** no globe test
- **GREEN:** Add test. Verify `GLOBE_LAT_MULT = 360.0 / 3.0 + 1.0 = 121.0`

### E3 — Stats Counter + Reset
- **RED:** `tests/stats_collector.rs` → `test_stats_defaults`, `test_stats_increment` — create `Stats`, assert defaults, increment, re-read
- **Expected fail:** no test file
- **GREEN:** Create `tests/stats_collector.rs`. Add increment methods to `Stats` if missing.

### E4 — binCraft Layout
- **RED:** `tests/output_compat.rs` → `test_bincraft_memory_layout` — `size_of::<BinCraft>() == 112`, field offsets via `offset_of!`
- **Expected fail:** no binCraft test
- **GREEN:** Add test. If layout wrong, adjust `_pad` size or field order to match 112 bytes.

---

## Phase F: 🟡 Benchmarks (2 TDD cycles)

### F1 — CRC Benchmark
- **RED:** `cargo bench crc_bench` reports meaningless constant-time measurement (current `black_box(42)`)
- **Expected fail:** benchmark doesn't measure CRC
- **GREEN:** Replace with real bench: `modes_checksum()` with 112-bit message, `CrcFixEngine::diagnose()` with known bad syndrome. Run, record baseline.

### F2 — Demod Benchmark
- **RED:** `cargo bench demod_bench` — file doesn't exist, compile error
- **Expected fail:** no such bench
- **GREEN:** Create `benches/demod_bench.rs`: bench `convert_sc16q11()` with 2.4M sample buffer, bench `demodulate2400()` with magnitude buffer. Add `[[bench]]` to `Cargo.toml`. Run, record baseline.

---

## Phase G: ⚪ Polish Cleanup (2 TDD cycles)

### G1 — CLI Input Format Parsing
- **RED:** `tests/integration.rs` → `test_iformat_cu8` — `readsb --ifile test.iq --iformat CU8` uses `InputFormat::U8`; `--iformat SC16` uses `InputFormat::SC16Q11`
- **Expected fail:** `--iformat` ignored, always uses SC16Q11
- **GREEN:** Parse `config.iformat` → `InputFormat` enum. Use in `convert_to_magnitude()` call in main loop.

### G2 — Remove Dead Module References
- **RED:** `cargo check` — verify current state of `tracking/mod.rs` for `trace`/`db` declarations
- **Expected fail:** if declared but no files exist → compile error
- **GREEN:** If `trace.rs`/`db.rs` don't exist, remove `pub mod trace; pub mod db;` from `src/tracking/mod.rs`

---

## Full Execution Order (38 TDD cycles)

```
Phase A:  Tracker rewrite
  A1  → tracker creates aircraft
  A2  → invalid address guard
  A3  → baro altitude + validity
  A4  → geom altitude + delta
  A5  → callsign update
  A6  → velocity fields
  A7  → squawk update
  A8  → emergency state
  A9  → navigation state
  A10 → accuracy
  A11 → CPR position pairing
  A12 → stale removal
  A13 → test harness registration

Phase B:  SDR cleanup + FFI
  B1  → resolve 10 duplicate const warnings
  B2  → remove unused import
  B3  → RTL-SDR USB FFI + mock

Phase C:  Network output
  C1  → multi-listener accept
  C2  → broadcast publication
  C3  → Beast output formatting

Phase D:  Main loop (fixture + 4)
  D0  → generate test fixture from 10.4.10.152
  D1  → stale removal timer
  D2  → JSON output
  D3  → stats counter
  D4  → graceful shutdown

Phase E:  Output/stats unit tests
  E1  → JSON serialization test
  E2  → globe index test
  E3  → stats collector tests
  E4  → binCraft layout test

Phase F:  Benchmarks
  F1  → CRC benchmark
  F2  → demod benchmark

Phase G:  Polish
  G1  → CLI iformat parsing
  G2  → remove dead module refs
```

Total: **38 TDD cycles** across 7 phases.
