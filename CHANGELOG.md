# Changelog

## [0.6.0] - 2026-05-21

### Fixed
- **Beast output framing**: MLAT sync header was `0x10 0x03` (DLE ETX) instead of
  `0x10 0x02` (DLE STX). Message type byte emitted raw DF type (0-31) instead of
  `0x31`/`0x32` for short/long frames. Both caused downstream clients to reject
  all data, showing zero aircraft. `src/net/protocols/beast.rs`
- **Aircraft `seen` never updated**: `update_from_message()` (every CRC-OK
  message) was missing `a.seen = now`, so `seen` stayed at creation time.
  Aircraft expired after 60 seconds regardless of continued message reception.
  `src/tracking/tracker.rs`
- **Signal level always 0.0**: Demodulator returned raw bytes with no signal
  info. `Aircraft::add_signal()` existed but was never called. `ModesMessage::signal_level`
  stayed at `0.0` default. `src/demod/demod_2400.rs`, `src/modes/parser.rs`,
  `src/main.rs`
- **`cpr_ok` stat always 0%**: `cpr_decoded` was never set to `true` in
  `decode_airborne_position` and `decode_surface_position`, so the stats
  accumulator counted all CPR as failed. `src/modes/parser.rs`
- **DF percentages >100%**: `messages_by_type` was cumulative since process
  start while `msgs_per_sec` denominator used a 60s window. Replaced with
  windowed `VecDeque<(Instant, u8)>`. `src/console/stats_accumulator.rs`
- **Receiver position unused**: `READSB_LAT`/`READSB_LON` parsed but never
  passed to tracker. `decode_cpr_relative()` existed but was never called.
  `src/main.rs`, `src/tracking/tracker.rs`

### Changed
- `TRACK_STALE` 15s → 60s (match readsb C)
- `TRACK_EXPIRE` 60s → 300s (match readsb C)

### Added
- **Signal level pipeline**: `demodulate2400` returns `Vec<(Vec<u8>, f64)>`
  with signal = average of 4 preamble peak magnitudes. Signal propagates
  through parser to tracker's `add_signal()` and `get_signal_db()`.
- **Haversine distance**: `pub fn haversine_distance()` in `src/tracking/mod.rs`
- **Receiver position wiring**: Config lat/lon passed to Tracker, used for
  relative CPR decode (instant position from single frame) and range filtering.
- **Range filtering**: `max_range` field on Tracker now enforced — positions
  beyond the configured distance are discarded.
- **33 new tests** across demod, parser, tracker, and stats accumulator modules.

## [0.5.0] - 2026-05-21

### Added
- **Tiered console output system**: new `src/console/` module (state.rs,
  formatter.rs, stats_accumulator.rs, outputter.rs) providing four verbosity
  levels controlled by `CONSOLE_LEVEL` env var and runtime SIGUSR1/SIGUSR2
  signals.
  - **Low**: aggregated stats line every 60s (msg rate, aircraft count, CRC
    quality, signal stats, DF distribution, CPR decode rate, uptime).
  - **Medium**: per-aircraft change summary every `CONSOLE_INTERVAL` seconds
    (default 10), showing only fields that changed since last report with
    directional indicators (▲/▼/▸/---).
  - **High**: compact per-message output for decoded ADS-B messages
    (skips DF11 All-Call and empty frames).
  - **Max**: every CRC-passing message including DF11/empty frames.
- **StatsAccumulator**: rolling 60-second window stats for display purposes,
  independent from the existing cumulative `Stats` struct.
- **30 unit tests** across all console module components, all pass.

### Changed
- `--version` from 0.4.0 to 0.5.0.

## [0.4.0] - 2026-05-20

### Added
- **Gain control now works**: `--gain` / `READSB_GAIN` actually sets the SDR
  tuner gain via `SdrManager::set_gain()` delegation to the underlying device.
- **Inbound protocol parsing**: Beast/Hex/SBS data received on network ports
  (30002/30003/30005) is now dispatched to the appropriate parser and fed into
  the tracker via a dedicated `incoming_tx` broadcast channel. Multi-receiver
  setups and mlat-client support enabled.
- **Periodic JSON output**: `--json-dir <path>` writes `aircraft.json` every
  `--json-reliable` seconds, with optional `--json-globe-index` for per-aircraft
  globe- partitioned JSON files.
- **Runtime stats collection**: `Stats` struct is wired into the main processing
  loop — samples processed, messages decoded, unique aircraft tracked — printed
  every 60 iterations.
- **Mode A/C demodulation**: `src/demod/demod_ac.rs` exposes `demodulate_ac()`
  and `modeac_to_altitude()` thin wrappers around the already-tested Mode A/C
  preamble detector. `Tracker::update_squawk_only()` added for squawk updates.
- **Comm-B aircraft identification**: `decode_comm_b()` is wired into the DF20
  dispatcher in `parser.rs`. New `commb_callsign()` helper extracts packed 6-bit
  AIS callsigns from Comm-B MB fields. `ModesMessage` gains a `commb_format`
  field.

### Changed
- `NetworkServer::new()` now takes `&[(&str, InputParser)]` tuples, associating
  a parser variant with each listen address. `NetworkServer` has a separate
  `incoming_tx` channel for inbound decoded messages.

### Removed
- `src/net/protocols/uat.rs` deleted (no C equivalent, never tested).
- `find_frame()` removed from `beast.rs` (Beast framing will be inline).
- Unused re-exports `pub use bincraft::*; pub use heatmap::*;` removed from
  `output/mod.rs`.

### Fixed
- Zero compiler warnings across all targets (20+ `#[allow(dead_code)]` annotations
  added to known incomplete feature stubs pending their integration phases).

## [0.3.5] - 2026-05-20

### Fixed
- **No data sent to network clients**: broadcast channel `message_tx.send()`
  was never called — decoded messages were tracked internally but never
  Beast-encoded and published. Clients connected to ports 30002/30003/30005
  received no data. Now each CRC-OK decode is Beast-encoded and broadcast.

## [0.3.4] - 2026-05-20

### Fixed
- **Crash when short (56-bit) message received**: `parse_modes_message`
  was called with hardcoded 112-bit message length; short messages (7
  bytes) caused `range end index 14 out of range for slice of length 7`.
  Now uses `raw_msg.len() * 8` to determine actual bit length.

### Added
- `RUST_BACKTRACE=1` set in `docker-entrypoint.sh` for panic diagnostics

## [0.3.3] - 2026-05-20

### Fixed
- **`--gain auto` crashes container**: entrypoint now skips `--gain` when
  value is `auto` (the docker-compose default), letting the binary use
  its built-in default gain (49.6 dB)

## [0.3.2] - 2026-05-20

### Added
- **Docker entrypoint script**: `docker-entrypoint.sh` maps environment
  variables to CLI arguments, enabling Docker Compose/Swarm configuration
  via env vars (`READSB_DEVICE`, `READSB_DEVICE_TYPE`, `READSB_NET`,
  `READSB_GAIN`, `READSB_PPM`, `READSB_LAT`, `READSB_LON`, etc.)
- **Env var configuration**: all CLI flags support `READSB_*` environment
  variables via clap `env` attribute (works outside Docker too)

### Changed
- Dockerfile entrypoint changed from binary to `docker-entrypoint.sh`

## [0.3.1] - 2026-05-20

### Fixed
- **CI clippy failures**: 15 new lints from Rust 1.95 toolchain resolved
  across 8 source files (io_other_error, needless_range_loop,
  field_reassign_with_default, len_without_is_empty, collapsible_if,
  unnecessary_cast, manual_is_multiple_of)
- **aarch64 build in CI**: missing `targets:` parameter on
  `dtolnay/rust-toolchain@stable` causing `can't find crate for core`
- **Docker build**: `COPY benches/ benches/` referenced directory excluded
  by `.dockerignore`; removed unnecessary COPY line

### Changed
- Multi-arch Docker image published to GHCR:
  `ghcr.io/dlasher/readsb-rs:latest` (main) and semver tags (v*)

## [0.3.0] - 2026-05-20

### Fixed
- **Sample rate mismatch**: decoder assumed 2 MHz but RTL-TCP/USB drivers set
  2.4 MHz, causing preamble timing to be off by 20% (16 samples = 6.67 µs vs
  required 8 µs). Changed defaults to 2000000.
- **Preamble detection used broken absolute threshold comparison**: replaced
  `mag[0] > t && mag[4] > t && ...` with the correct relative-comparison
  algorithm from original dump1090 (checks `mag[0] > mag[1]`, `mag[2] > mag[3]`,
  gap-vs-peak energy ratios, etc.)
- **Decode message phase default returned `false`**: the phase cycles through
  0,6,12,18,24 but the match arm `_ => false` meant only 1 in 5 bits was
  decoded. Changed to `_ => slice_phase0(slice) > 0` matching original C code.
- **Demod loop off-by-one**: `while i + 240 < mag_len` prevented processing
  messages at the exact buffer boundary. Changed `<` to `<=`.
- **EOF not handled**: `read_samples` returning `Ok(0)` (file EOF / TCP close)
  caused the main loop to spin forever. Added explicit `Ok(0) => break`.
- **`InputFormat` not `Copy`**: move-in-loop error when matching on `input_format`
  in the main processing loop. Added `derive(Clone, Copy)`.
- **Synthetic fixture had wrong amplitudes**: used `amplitude * 2047` producing
  magnitudes below any detection threshold. Switched to full i16 range
  (pulse=30000, quiet=2000) with guard sample for decoder window.

### Changed
- Default sample rate from 2400000 to 2000000 in both `RtlSdrDevice` and
  `RtlTcpClient`
- Sample rate in fixture generator from 2400000 to 2000000

## [0.2.0] - 2026-05-20

### Added
- Full tracker implementation with per-field DataValidity integration:
  barometric/geometric altitude, callsign, velocity, squawk, emergency,
  navigation state, accuracy, CPR position pairing, stale removal
- Network multi-listener support: server accepts connections on all
  bound ports concurrently via `futures::future::select_all`
- Broadcast publication: decoded messages are written back to all
  connected clients in Beast binary format
- Beast output encoder: DLE/ETX framing with 6-byte timestamp and
  message type byte
- SDR mock device (`SdrType::Mock`) for testing without hardware
- 49 new tests across tracker, network, output, stats, SDR, config,
  and binary-level integration (83 total, up from 34)
- CRC and demodulation benchmarks (replaced placeholder)
- GitHub Actions release workflow: builds x86_64 + aarch64 Linux
  binaries, Docker image, publishes to GitHub Releases on `v*` tag push
- Live RTL-TCP test fixture (sampled from 10.4.10.152:2345)
- Synthetic DF17 burst fixture for deterministic testing
- CHANGELOG.md

### Changed
- `Tracker::update_from_message()` now returns `Option<Arc<RwLock<Aircraft>>>`
  instead of `()`, with invalid address (0/0xFFFFFF) returning `None`
- Tracker methods replaced inline field updates with dedicated per-field
  updaters (`update_altitude`, `update_callsign`, `update_velocity`,
  `update_squawk`, `update_emergency`, `update_nav`, `update_accuracy`,
  `update_position`, `remove_stale`)
- `ClientConnection::handle()` signature now takes both broadcast
  `Sender` and `Receiver` for bidirectional message flow
- `NetworkServer::with_channel()` added for explicit broadcast channel
  injection; `message_tx` made public
- RTL-SDR USB backend returns `Err(Unsupported)` instead of panicking
  with `unimplemented!()`
- Dockerfile now includes `librtlsdr0` and `libusb-1.0-0` runtime deps;
  builder stage installs `libusb-1.0-0-dev` for rtl-sdr-rs crate
- `.dockerignore` excludes tests/, examples/, benches/, docs/,
  test_fixtures/

### Fixed
- Resolved 10 ambiguous glob re-export warnings by removing duplicate
  `DongleInfo`/`RTLTCP_*` constants from `src/sdr/rtlsdr.rs` (single
  source in `src/sdr/rtl_tcp.rs`)
- Resolved unused `Arc` import warning in tracker

### Removed
- `RtlSdrDevice::with_rtl_tcp()`, `set_direct_samp()`, `set_offset_tune()`,
  `set_bias_tee()` (rtl_tcp-specific logic removed from rtlsdr.rs —
  `RtlTcpClient` in `rtl_tcp.rs` is the single source)

## [0.1.0] - 2026-05-20

### Added
- Initial project scaffold with core type definitions
- CRC-24 computation and error correction (single/double bit fix) with
  compatibility tests against C reference
- CPR decoding (airborne, surface, relative) with cross-validation tests
- Mode S message parser supporting all DF types with ME type dispatch
- Comm-B decoder, Mode A/C detection, AIS charset
- Aircraft struct and thread-safe `AircraftRegistry`
- Data validity system with staleness tracking and source priority
- I/Q format conversion (SC16Q11, U8, F32)
- 2.4MHz Mode S demodulator with preamble detection and Manchester decoding
- SDR hardware abstraction with RTL-TCP client and RTL-SDR USB placeholder
- TCP server with tokio listeners and Beast/SBS/Hex/UAT protocol parsers
- JSON serialization, globe index, binCraft, heatmap, statistics modules
- CLI configuration with clap (80+ options)
- Async main loop wiring all subsystems (SDR → demod → parse → track)
- Dockerfile and GitHub Actions CI (test + build + docker)
