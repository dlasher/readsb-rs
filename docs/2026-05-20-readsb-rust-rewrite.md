# readsb → Rust: Complete Implementation Plan (Part 1/2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite readsb (45K LOC C ADS-B/Mode-S receiver) in Rust — memory safety, equal performance, better stability.

**Architecture:** Bottom-up phased rewrite in 8 phases. Phases 1-3 are pure Rust (no FFI). Phase 5 adds SDR FFI. Each phase produces testable, working software.

**Tech Stack:** Rust 2021 edition, `tokio`, `serde`/`serde_json`, `clap`, `tracing`, `zstd`/`flate2`, `bindgen` (C FFI), `criterion` (benchmarks).

**Repository Layout:**
- **C source (read-only reference):** `/CODE/readsb/` — original 45K LOC codebase, remains untouched
- **Rust project (new):** `/CODE/readsb-rs/` — isolated project root, clean `git init`, independent GitHub repo
- **Plan document:** Saved here at `.opencode/plans/2026-05-20-readsb-rust-rewrite-p1.md` + mirrored to `/CODE/readsb-rs/docs/`

**C codebase analyzed at tag:** v3.16.17 (last 30 commits reviewed for plan currency)

**Cross-validation commands** reference the C source at `/CODE/readsb/`, e.g.:
```bash
cd /CODE/readsb && make crctests && ./crctests   # validate CRC
cd /CODE/readsb && make cprtests && ./cprtests   # validate CPR
```

---

## Scope Decomposition

| Phase | Subsystem | C LOC | Risk | FFI? |
|-------|-----------|-------|------|------|
| 1 | CRC + Error Correction | 566 | Low | No |
| 2 | CPR Decoding | 374 | Low | No |
| 3 | Mode S Parsing + Comm-B | 4,200 | Medium | No |
| 4 | Aircraft Tracking | 6,500 | Medium | No |
| 5 | Demodulation + SDR | 5,500 | High | Yes |
| 6 | Network I/O | 9,200 | Medium | No |
| 7 | Output + Globe Index | 7,300 | Low | No |
| 8 | Main Loop + Integration | 6,100 | Medium | No |

---

## Project Structure

```
/CODE/readsb-rs/                     ← isolated Rust project root (clean git repo)
├── Cargo.toml
├── README.md                        ← documents the Rust rewrite
├── docs/                            ← plan + reference docs (self-contained)
│   └── 2026-05-20-readsb-rust-rewrite.md
├── tests/ {crc_compat, cpr_compat, modes_compat, tracking_compat, demod_compat}/
├── benches/ {crc_bench, demod_bench}.rs
└── src/
    ├── main.rs          # Phase 8
    ├── lib.rs
    ├── config.rs        # Phase 8
    ├── types/           # Phase 1: Core types (readsb.h enums)
    │   ├── mod.rs, address.rs, altitude.rs, message.rs
    │   ├── accuracy.rs, status.rs, comm_b.rs
    ├── crc/             # Phase 1: CRC-24 + error correction
    │   ├── mod.rs, engine.rs, fix.rs
    ├── cpr/             # Phase 2: Compact Position Reporting
    │   ├── mod.rs, constants.rs, decode.rs
    ├── modes/           # Phase 3: Mode S parsing
    │   ├── mod.rs, parser.rs, comm_b.rs, mode_ac.rs, ais.rs
    ├── tracking/        # Phase 4: Aircraft state tracking
    │   ├── mod.rs, validity.rs, aircraft.rs, tracker.rs, trace.rs, db.rs
    ├── demod/           # Phase 5: 2.4MHz demodulator
    │   ├── mod.rs, convert.rs, demod_2400.rs
    ├── sdr/             # Phase 5: SDR hardware abstraction
    │   ├── mod.rs, traits.rs, ifile.rs, rtlsdr.rs, manager.rs
    ├── net/             # Phase 6: TCP networking
    │   ├── mod.rs, server.rs, client.rs
    │   └── protocols/ {beast.rs, sbs.rs, hex.rs, uat.rs}
    ├── output/          # Phase 7: JSON, globe, heatmap
    │   ├── mod.rs, json.rs, globe.rs, bincraft.rs, heatmap.rs
    └── stats/           # Phase 7: Statistics
        ├── mod.rs, collector.rs
```

## Task Summary

Total **35 tasks** across 8 phases. Each task takes ~15-30 min (TDD: write test → fail → implement → pass → commit).

---

## Phase 1: CRC + Error Correction (566 C LOC, Low Risk, No FFI)

### Task 1.1: Project Setup + Core Types
**Files:** `Cargo.toml`, `src/lib.rs`, `src/types/mod.rs`, `src/types/address.rs`, `src/types/altitude.rs`, `src/types/status.rs`
- [ ] Create Cargo.toml with deps: clap, serde, serde_json, tokio, tracing, zstd, flate2, thiserror, bitflags
- [ ] Create src/lib.rs + types/mod.rs with re-exports
- [ ] Create address.rs: `DataSource` (Invalid=0..Priority=12) and `AddrType` (AdsbIcao=0..Unknown=13) enums matching readsb.h:163-203
- [ ] Create altitude.rs: `AltitudeUnit {Feet,Meters}`, `AltitudeSource {Baro,Geom}`, `INVALID_ALTITUDE = -9999`
- [ ] Create status.rs: `AirGround`, `Emergency`, `SilType`, `CprType`, `HeadingType`, `NavAltitudeSource` enums + `NavModes` bitflags
- [ ] `cargo check` then commit

### Task 1.2: CRC-24 Computation Engine
**Files:** `src/crc/mod.rs`, `src/crc/engine.rs`, `tests/crc_compat/test_crc.rs`
- [ ] Create `modes_checksum(msg, bitlen)` — bit-serial CRC-24 with polynomial 0xFFF409, matching crc.c exactly
- [ ] Write tests: valid long message (112 bits → CRC=0), valid short message (56 bits), corrupted message (non-zero CRC), all-zeros
- [ ] Cross-validate: build C crctests (`make crctests && ./crctests`), compare syndrome outputs
- [ ] `cargo test crc_compat` then commit

### Task 1.3: CRC Error Correction Tables
**Files:** `src/crc/fix.rs`, `tests/crc_compat/test_crc_fix.rs`
- [ ] Implement `CrcFixEngine::new(max_bitlen)` — precompute syndrome table for all single + double bit errors (MODES_MAX_BITERRORS=2)
- [ ] Implement `diagnose(syndrome)` → `Option<ErrorInfo>` and `fix(msg, info)` → flips bits
- [ ] Write tests: single-bit fix, double-bit fix, verify CRC=0 after fix
- [ ] `cargo test` then commit

---

## Phase 2: CPR Decoding (374 C LOC, Low Risk, No FFI)

### Task 2.1: CPR Module + Constants
**Files:** `src/cpr/mod.rs`, `src/cpr/constants.rs`
- [ ] Create cpr module with `CPR_NZ=15`, `CPR_AIRBORNE_RES=131072.0`, `CPR_SURFACE_RES=16384.0`, `NL_TABLE[59]`

### Task 2.2: CPR Decode Functions
**Files:** `src/cpr/decode.rs`, `tests/cpr_compat/test_cpr.rs`
- [ ] Implement `decode_cpr_airborne()` — 4-arg even/odd pair → `(lat, lon)`. Algorithm: compute j = floor(59*even - 15*odd), resolve latitude zone, compute NL, compute longitude zone. Return normalized coordinates.
- [ ] Implement `decode_cpr_surface()` — same but with reference lat/lon, surface resolution
- [ ] Implement `decode_cpr_relative()` — single frame + reference point
- [ ] Helper: `cpr_nl(lat)`, `normalize_lat()`, `normalize_lon()`
- [ ] Run C `make cprtests && ./cprtests` for reference test vectors
- [ ] Write tests: basic airborne decode, basic surface decode, zero values → None
- [ ] `cargo test cpr_compat` then commit

---

## Phase 3: Mode S Parsing (4,200 C LOC, Medium Risk, No FFI)

### Task 3.1: Message Struct
**Files:** `src/types/accuracy.rs`, `src/types/comm_b.rs`, `src/types/message.rs`
- [ ] `MessageAccuracy` struct: nic_a..nic_baro validity + values, nac_p/v, sil, gva, sda
- [ ] `OpStatus` struct: version, sil_type, all capability flags (om_*, cc_*)
- [ ] `NavState` struct: fms altitude, mcp altitude, qnh, heading, nav modes
- [ ] `CommBFormat` enum: Unknown, EmptyResponse, AircraftIdent, AcasRA, VerticalIntent, etc.
- [ ] `ModesMessage` struct (100+ fields): mirror `struct modesMessage` from readsb.h:990-1247 — msg bytes, verbatim bytes, timing, source, addr, all extracted fields, decoded values

### Task 3.2: Message Parser
**Files:** `src/modes/mod.rs`, `src/modes/parser.rs`, `tests/modes_compat/test_parser.rs`
- [ ] `parse_modes_message(msg, msgbits, crc_engine)` → `Option<ParseResult>`: extract DF type, CRC check + fix, dispatch to per-DF decoder
- [ ] Common fields: CA (capability), CF (format code), AA (address)
- [ ] DF 0/4/16 (Altitude): extract AC field, call `decode_altitude()`
- [ ] DF 5/21 (Squawk): extract 13-bit identity, encode to hex squawk
- [ ] DF 11 (All-Call): address + CA only
- [ ] DF 17/18 (ADS-B): extract ME bytes, decode by ME type:
  - Types 1-4: surface position (CPR + movement table + air/ground flag)
  - Types 5-8: airborne position (CPR + Q-bit altitude encoding)
  - Types 9-19: airborne velocity (EW/NS components, vertical rate)
  - Type 20: target state (MCP altitude)
  - Types 21/28/31: aircraft status (emergency codes)
- [ ] DF 19 (Military): extract AF field
- [ ] DF 20 (Comm-B): altitude + MB bytes
- [ ] DF 31 (ACAS): MV bytes
- [ ] Tests: known DF17 message `8D4840D6202CC371C32CE0576098`, DF11 short message, invalid length
- [ ] `cargo test modes_compat` then commit

### Task 3.3: Comm-B, Mode A/C, AIS
**Files:** `src/modes/comm_b.rs`, `src/modes/mode_ac.rs`, `src/modes/ais.rs`
- [ ] Comm-B: `decode_comm_b(mb)` → `CommBFormat` — dispatch by BDS register (0x10=AirIdent, 0x30=ACAS RA, 0x40=VerticalIntent, 0x80=Meteorological)
- [ ] Mode A/C: `detect_mode_a(samples)` → 13-bit code via preamble + pulse detection; `mode_a_to_mode_c(modeA)` → Gillham altitude conversion
- [ ] AIS: 64-character charset table from ais_charset.c
- [ ] `cargo test` then commit

---

## Phase 4: Aircraft Tracking (6,500 C LOC, Medium Risk, No FFI)

### Task 4.1: Data Validity System
**Files:** `src/tracking/validity.rs`, `src/tracking/mod.rs`
- [ ] `DataValidity` struct: updated, next_reduce_forward, source, last_source, stale — mirrors track.h:107
- [ ] `update(source, now)`: only accept if source >= current priority
- [ ] `is_valid(now, expiration)`: check if not expired (JAERO=33min, else configurable)
- [ ] `check_stale(now)`: set stale if age > TRACK_STALE (15s)
- [ ] Tests: fresh data is valid, data goes stale after 15s, data expires at expiration, source priority ordering

### Task 4.2: Aircraft Struct + Thread-Safe Registry
**Files:** `src/tracking/aircraft.rs`, `tests/tracking_compat/test_aircraft.rs`
- [ ] `Aircraft` struct (50+ fields): addr, addrtype, seen, position, altitude, velocity, identity, CPR state, validity trackers, signal, NAV — mirrors track.h:313
- [ ] `Aircraft::new(addr, addrtype, now)` — initialize with defaults
- [ ] `add_signal(level, now)` — circular buffer of 8 levels
- [ ] `get_signal_db()` — average → dB conversion
- [ ] `AircraftRegistry` — `RwLock<HashMap<u32, Arc<RwLock<Aircraft>>>>`
- [ ] `get_or_create()` — entry API for atomic get-or-insert
- [ ] `remove_stale(now, expire)` — retain filter
- [ ] Tests: creation, signal averaging, registry concurrent access (10 threads), stale removal

### Task 4.3: Tracker — State Updates from Messages
**Files:** `src/tracking/tracker.rs`, `tests/tracking_compat/test_tracker.rs`
- [ ] `Tracker` struct: AircraftRegistry + config (json_reliable, max_range, user position)
- [ ] `update_from_message(mm, now)` → `Option<Arc<RwLock<Aircraft>>>`: entry point, validates address, creates aircraft, dispatches per-field updates
- [ ] Per-field updaters: callsign, altitude (baro + geom + delta), velocity (gs, ias, tas, mach), position (CPR even/odd pair → decode), squawk, category, emergency, NAV (mcp, fms, qnh), accuracy (nic, nac)
- [ ] Position decoding: even+odd CPR pairing → `decode_cpr_airborne()` or `decode_cpr_surface()` → range check
- [ ] Tests: aircraft creation from message, altitude update, invalid address ignored, message count tracking

---

## Phase 5: Demodulation + SDR (5,500 C LOC, High Risk, FFI)

### Task 5.1: I/Q Format Conversion
**Files:** `src/demod/mod.rs`, `src/demod/convert.rs`, `tests/demod_compat/test_convert.rs`
- [ ] `InputFormat` enum: SC16Q11, SC16Q11M, F32, U8
- [ ] `convert_to_magnitude(input, format, output) → count`: dispatch to format-specific converter
- [ ] SC16Q11: 16-bit signed I/Q interleaved, `sqrt(I²+Q²)` per sample, scale to uint16
- [ ] U8: 8-bit unsigned, subtract 128 for zero-IF centering
- [ ] F32: 32-bit float, scale to uint16 range
- [ ] Tests: known input → expected magnitude, empty input → 0 samples

### Task 5.2: 2.4MHz Demodulator (Hot Path)
**Files:** `src/demod/demod_2400.rs`
**C ref:** `demod_2400.c` had ~93 lines of Mode A/C debug drawing code (gd.h dependency) removed since v3.16.17. The signature changed from `demodulate2400(mag)` to `demodulate2400(mag, mm_buf)` — message buffer is now passed explicitly instead of pulled from global state. The correlation functions, preamble detection, and Manchester decode logic are unchanged.
- [ ] Correlation functions: `slice_phase0..4(m)` — Manchester-encoded bit correlation at 5 phase offsets (matching demod_2400.c coefficients: 18×m0 - 15×m1 - 3×m2, etc.)
- [ ] `check_preamble(mag, threshold)` — detect Mode S preamble: pulses at 0, 2, 7, 9 sample positions, quiet at 1, 5
- [ ] `decode_message(mag, bitlen)` — Manchester decode: for each bit, apply phase-appropriate correlation function, accumulate bit into message byte
- [ ] `demodulate2400(mag, len, threshold)` → `Vec<Vec<u8>>`: scan buffer for preambles, decode long (112 bit) or short (56 bit) messages
- [ ] Tests: pure noise → no messages, empty buffer → empty, small buffer → empty

### Task 5.3: SDR Hardware Abstraction
**Files:** `src/sdr/mod.rs`, `src/sdr/traits.rs`, `src/sdr/ifile.rs`, `src/sdr/rtlsdr.rs`, `src/sdr/manager.rs`
**C ref:** `sdr_rtlsdr.c` added ~531 lines for rtl_tcp client support (v3.16.16). TCP socket connection, protocol handshake, read thread, auto-reconnect. `readsb.h` added `gain_stats_t` (loudEvents, noiseLowSamples, noiseHighSamples, totalSamples) and `agc_state_t` (slowRise, nextRaiseAgc, loudRebound) for AGC gain statistics.
- [ ] `SdrDevice` trait: `async open/close/set_freq/set_gain/set_sample_rate/read_samples`
- [ ] `IFileDevice`: reads raw I/Q from file via `tokio::fs::File`
- [ ] `RtlSdrDevice`: FFI to librtlsdr via rtlsdr-sys crate, with rtl_tcp mode support:
  - `dongle_info_t` struct: `magic[4]="RTL0"`, `tuner_type(u32)`, `tuner_gain_count(u32)` — all network byte order
  - `rtltcp_command` struct: `cmd(u8)`, `param(u32 BE)` — packed
  - Command codes: `RTLTCP_SET_FREQ=0x01`, `SET_SAMPLE_RATE=0x02`, `SET_GAIN_MODE=0x03`, `SET_GAIN=0x04`, `SET_FREQ_CORR=0x05`, `SET_IF_GAIN=0x06`, `SET_DIRECT_SAMP=0x09`, `SET_OFFSET_TUNING=0x0A`, `SET_BIAS_TEE=0x0E`
  - Auto-reconnect: close, wait 5s, retry connect loop
  - Read thread: TCP recv into ring buffer, signal callback
- [ ] `SdrManager`: factory + lifecycle for SDR devices
- [ ] Gain stats structs: `GainStats { loud_events, noise_low, noise_high, total }`, `AgcState { slow_rise, next_raise_agc, loud_rebound }`
- [ ] `cargo check` (full implementation requires rtlsdr-sys crate)

---

## Phase 6: Network I/O (9,200 C LOC, Medium Risk, No FFI)

**C ref:** `net_io.c` added lazy sendq compaction (v3.16.17) — uses offset-based memmove instead of O(n) per partial write. Added `simple_drain` flag to `struct messageBuffer` and `netDrainBuffer()` for single-buffer draining. Avoids global `netDrainMessageBuffers()` call.

### Task 6.1: TCP Server + Client
**Files:** `src/net/mod.rs`, `src/net/server.rs`, `src/net/client.rs`
- [ ] `NetworkServer`: manages multiple tokio TcpListeners, broadcast channel for decoded messages
- [ ] `add_listener(bind_addr)`: bind TCP listener
- [ ] `ClientConnection::handle()`: read from socket, detect protocol, dispatch to parser
- [ ] Only reads/processes — Phase 8 adds write-back
- [ ] Send queue: Rust `BytesMut` handles zero-copy buffering natively — no need for C's manual offset-based compaction

### Task 6.2: Protocol Parsers
**Files:** `src/net/protocols/{mod,beast,sbs,hex,uat}.rs`
- [ ] Beast: DLE (0x10) ETX (0x03) framing, 6-byte timestamp, message type + payload
- [ ] SBS/Basestation: CSV with 22 fields (MSG, type, callsign, ICAO, alt, speed, track, lat, lon, etc.)
- [ ] Hex: `@...;` (DF11) or `*...;` (DF17/18) hex string format
- [ ] UAT: UAT 978MHz packet format parser (reference uat2esnt/*.c)

---

## Phase 7: Output + Globe Index + Stats (7,300 C LOC, Low Risk, No FFI)

### Task 7.1: JSON Serialization
**Files:** `src/output/json.rs`
- [ ] `serde::Serialize` on Aircraft struct
- [ ] Generate aircraft.json (list of all aircraft with lat/lon/alt/callsign/gs/track/emergency)
- [ ] Generate status.json (receiver stats)
- [ ] Configurable fields (include_nopos, wind_triggered, trace)

### Task 7.2: Globe Indexing
**Files:** `src/output/globe.rs`
- [ ] `globe_index(lat, lon)` → tile index: 3° grid, GLOBE_LAT_MULT for lat zone
- [ ] Globe JSON writer: batch aircraft by tile
- [ ] Trace system: `struct state` (packed 48-bit timestamp + lat/lon/alt/gs), point compression, per-aircraft trace chunks

### Task 7.3: binCraft + Heatmap + Statistics
**Files:** `src/output/bincraft.rs`, `src/output/heatmap.rs`, `src/stats/collector.rs`
- [ ] `binCraft` packed format (112 bytes): hex, seen, lon, lat, alt, gs, track, callsign, squawk, nav — fixed layout for JavaScript Int32Array
- [ ] Heatmap: tiled heat entries (hex, lat, lon, alt, gs)
- [ ] Statistics: demod counts (preambles, accepted, rejected), CPR success/failure counts, CPU timing per subsystem, range histogram (128 buckets), network bytes in/out

---

## Phase 8: Main Loop + Integration (6,100 C LOC, Medium Risk, No FFI)

### Task 8.1: Configuration + CLI
**Files:** `src/config.rs`
**C ref:** v3.16.16 added rtl_tcp CLI options. `readsb.h` added `gain_stats_t`, `agc_state_t` structs. New test files available for cross-validation: `test_affinity.c`, `test_gainstats.c`, `test_receiver.c`, `test_ringbuf.c`, `test_sprint.c`, `test_threadpool.c`.
- [ ] `ReadsbConfig` struct parsed via `clap` derive: mirrors all `--` options from readsb.c
- [ ] 80+ options: `--device-type`, `--device`, `--gain`, `--freq`, `--net`, `--json-dir`, `--lat`, `--lon`, `--max-range`, SDR-specific opts, debug flags
- [ ] **rtl_tcp options:** `--rtltcp-direct-samp=<mode>` (0=off, 1=I-ADC, 2=Q-ADC), `--rtltcp-offset-tune=<0|1>`, `--rtltcp-bias-tee=<0|1>`
- [ ] `Config::from_cli()` → `ReadsbConfig` with argp emulation

### Task 8.2: Main Loop
**Files:** `src/main.rs`
- [ ] Initialize: parse CLI → open SDR device → bind TCP ports → create tracker → signal handlers
- [ ] `run()` async main loop:
  1. Read I/Q samples from SDR
  2. Convert to magnitude
  3. Demodulate → raw messages
  4. Parse messages (CRC + field extraction)
  5. Update tracking state
  6. Generate JSON/gzipped output
  7. Handle TCP I/O (accept, read, write)
  8. Remove stale aircraft (every 1s)
  9. Collect statistics (every 10s)
  10. Handle clean shutdown on SIGTERM/SIGINT

### Task 8.3: Docker + CI
**C ref:** v3.16.16 added `Dockerfile.rtl_tcp`, `Dockerfile.soapy`, `docker-entrypoint.sh` (ENV-to-CLI-arg translation). Multi-arch GitHub Actions workflow at `.github/workflows/docker.yaml`.
- [ ] Dockerfile: multi-stage build (musl target or debian:bookworm-slim)
- [ ] Variants: standard, rtl_tcp client (no librtlsdr needed server-side), SoapySDR
- [ ] GitHub Actions: build + test + lint + multi-arch (linux/amd64, linux/arm64)
- [ ] Performance regression benchmark suite (reference C tests: `test_affinity`, `test_threadpool`, `test_receiver`, `test_ringbuf`, `test_gainstats`, `test_sprint`)

---

## Cross-Phase Concerns

- **Test fixtures:** Collect sample I/Q recordings + known-good message dumps from C readsb for regression testing. New C test files (`test_affinity.c`, `test_gainstats.c`, `test_receiver.c`, `test_ringbuf.c`, `test_sprint.c`, `test_threadpool.c`) provide additional reference vectors.
- **Benchmarks:** Use `criterion` for CRC, CPR, and demod hot paths; compare per-iteration time to C version
- **Memory:** Replicate C's hugepage-aware allocator pattern? Use `jemalloc` allocator via `tikv-jemallocator` crate
- **Safety audit:** Every `unwrap()` in phases 1-4 should become proper error handling in Phase 8

## Self-Review Checklist

- [ ] 1. **Spec coverage:** Every C module has a corresponding Rust module. All 8 phases produce testable software.
- [ ] 2. **No placeholders:** All code in the plan is complete Rust code, not TBD/TODO.
- [ ] 3. **Type consistency:** `DataSource`, `AddrType`, `ModesMessage` types are defined in Phase 1/3 and used consistently through Phase 8. `TrackEXPRE` constants match between validity and tracker. `decode_altitude()` signature is used identically in parser and tracking.
- [ ] 4. **Post-v3.16.17 review completed:** All recent changes evaluated and reflected above — gd.h removal (Phase 5), rtl_tcp protocol details (Phase 5), lazy sendq / simple_drain / netDrainBuffer (Phase 6), new CLI options + test files + AGC structs (Phase 8), multi-arch Docker (Phase 8). No structural changes to the 8-phase decomposition were needed.

## Execution Options

**Plan complete. Two execution options:**

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
