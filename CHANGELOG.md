# Changelog

## [0.11.1] - 2026-05-29

### Fixed
- **Squawk (identity) decoding used wrong formula**: `decode_df5_21()` used an incorrect bit-shift formula instead of the Gillham hex encoder `decode_id13()`. All DF5/DF21 squawk values were decoded incorrectly (0/1000 random test values matched the C reference). `src/modes/parser.rs`
- **Ground speed (movement) decoding used flat identity table**: `MOVEMENT_TABLE` was `[0.0, 1.0, ..., 124.0]` instead of the proper ADS-B piecewise-linear exponential scale (`decodeMovementFieldV0`/`V2`). All surface movement ground speeds were wrong (e.g. code 124 = 180 kt, not 124 kt). `src/modes/parser.rs`
- **CPR global decode hardcoded fflag=0**: Always passed even-frame flag to `decode_cpr_airborne()` instead of the current message's `cpr_odd` flag. Odd-frame position pairs produced incorrect positions. `src/tracking/tracker.rs`
- **CRC error correction included DF type bits**: Syndrome table construction started at bit 0 instead of bit 5, causing the CRC fixer to attempt corrections in the DF type field (handled separately via `fixDF17msgtype()` in the C reference). `src/crc/fix.rs`
- **Altitude decoding dropped Gillham Mode C (Q-bit=0)**: When the Q-bit was 0, altitude returned `INVALID_ALTITUDE` instead of performing Gillham Mode C decoding via `decode_id13()` → `mode_a_to_mode_c()`. Many older transponders use Gillham encoding. `src/modes/parser.rs`
- **Airborne velocity missing subtypes 3-4**: Decoder only handled subtypes 1-2 (EW/NS velocity components). Subtypes 3-4 provide heading + airspeed (IAS/TAS). Now extracts heading, IAS, TAS, NACv, and baro/geo delta. `src/modes/parser.rs`
- **DF17 type correction not implemented**: `fixDF17msgtype()` from the C reference was missing — messages with single-bit errors in the DF type field (DF=1,25,21,19,16 that could actually be DF17) were discarded instead of repaired. `src/demod/demod_2400.rs`
- **CA field never used for air/ground determination**: The CA (Capability) field was read but never mapped to air/ground state (CA=4 → Ground, CA=5 → Airborne). Only sets airground when currently `Invalid` to avoid overriding surface position determination. `src/modes/parser.rs`

## [0.11.0] - 2026-05-29

### Fixed
- **Callsign never decoded**: `commb_callsign()` existed but was never called from DF20 Comm-B. ADS-B TC 1-4 (Aircraft Identification) was dispatched to `decode_surface_position` instead of `decode_aircraft_identification`. Callsigns now extracted from both DF20 Comm-B AircraftIdent (BDS 2,0) and ADS-B TC 1-4 messages. `src/modes/parser.rs`
- **Heading/track never computed**: `track_valid` was declared but never set. `decode_airborne_velocity` computed ground speed but not heading. Now computes `atan2(ew, ns)` from signed EW/NS velocity components with proper sign handling. `src/modes/parser.rs`
- **Altitude always 0 in viewsb**: `decode_airborne_position` sets either `baro_alt` or `geom_alt` depending on the Q-bit. viewsb formatter only checked `baro_alt`. Now falls back to `geom_alt` when `baro_alt` is invalid. `src/viewsb/formatter.rs`
- **Tracker set heading to ground speed**: `a.track = msg.gs` assigned ground speed to the heading field. Now uses `msg.track`. `src/tracking/tracker.rs`
- **Console track used `cf*90` hack**: Console state and formatter computed track as `(msg.cf as f32) * 90.0` instead of using decoded heading. Now uses `msg.track`. `src/console/state.rs`, `src/console/formatter.rs`
- **Squawk never extracted from TC=28 Aircraft Status**: `decode_aircraft_status` only handled emergency type, missing the 13-bit Gillham squawk field. Now extracts squawk from TC=28 mesub=1 (bits 12-24) and TC=28 mesub=7 (bits 9-21) via `decode_id13()`. `src/modes/parser.rs`
- **Squawk never extracted from Mode A/C frames**: BEAST parser rejected 0x31 (Mode A/C) frames. `parse_modes_message` rejected 16-bit payloads. Now accepts Mode A/C frames and decodes squawk via `decode_mode_ac()` (Mode A code masked with `0x7777`). `src/net/protocols/beast.rs`, `src/modes/parser.rs`
- **DF5/21 Address/Parity frames corrupted by CRC fixer**: For DF0/4/5 (Address/Parity frames), the CRC syndrome IS the sender's ICAO address. The CRC fixer misinterpreted non-zero CRC as bit errors. Now handles Address/Parity frames by setting `mm.addr = crc` and skipping the fixer. `src/modes/parser.rs`
- **Airborne velocity bit positions completely wrong**: EW/NS velocity extraction used LSB-first bit numbering instead of MSB-first (C `getbits` convention). EW at bits 15-24, NS at bits 26-35, VR at bits 37-46. Sign convention also reversed (bit=1 → negative, matching C). Speeds now show correct subsonic values (~300-450 kt for cruise). `src/modes/parser.rs`
- **Airborne position altitude encoding wrong**: `decode_airborne_position` extracted altitude from `me[5]/me[6]` (surface position format) instead of `me[1]/me[2]` (12-bit field at ME bits 9-20). Now uses proper Q-bit (bit 4) and `n*25-1000` formula matching readsb-C `decodeAC12Field`. `src/modes/parser.rs`
- **CPR lat/lon extraction in airborne position**: Used wrong byte positions. Now extracts CPR lat from `me[2-4]` and CPR lon from `me[4-6]` per ADS-B standard. `src/modes/parser.rs`
- **viewsb formatter column alignment**: Seen column `{:>3}` truncated at 999 messages, RSSI `{:>5.1}` overflowed for strong signals. Now `{:>4}` for Seen, `{:>5.1}` for RSSI with proper spacing. `src/viewsb/formatter.rs`
- **viewsb screen staircase display**: `writeln!` with `\n` alone doesn't return to column 0 in crossterm raw mode. Now uses `write!` with `\r\n`. `src/viewsb/screen.rs`

### Added
- **`track: f32` field to `ModesMessage`**: Stores decoded heading from airborne velocity messages. `src/types/message.rs`
- **`decode_mode_ac()`**: Decodes Mode A/C frames (2-byte raw Mode A code) into squawk via `0x7777` mask with fudged non-ICAO address. `src/modes/parser.rs`
- **`decode_id13()`**: 13-bit Gillham code converter matching C reference, used for squawk extraction from TC=28/31 messages. `src/modes/parser.rs`
- **`decode_aircraft_identification()`**: Extracts 8-char AIS-6 callsign from ADS-B TC 1-4 messages. `src/modes/parser.rs`

### Changed
- Test count: 233 → 237

## [0.10.0] - 2026-05-28

### Added
- **`beast-client viewsb` subcommand**: Interactive terminal aircraft table (like `viewadsb` from readsb-C). Connects to any Beast source and displays a live-updating table of tracked aircraft using crossterm for TTY management. Supports interactive (alternate screen, keyboard quit) and non-interactive (pipe/file) modes. `src/viewsb/` module (6 files, 24 tests).
- **`beast-client viewsb --json` / `--csv` output**: Line-oriented JSON and CSV snapshot writers with atomic file writes. `src/viewsb/snapshot.rs`
- **`beast-client viewsb --sort`**: Sort by any column (icao, flight, alt, speed, heading, distance, seen) with ICAO tie-breaker for visual stability. Distance sort auto-enabled when `--lat`/`--lon` provided. `src/viewsb/sort.rs`
- **`beast-client viewsb --metric`**: Metric unit conversion (m, km/h, km) for altitude, speed, and distance columns. `src/viewsb/formatter.rs`
- **`beast-client viewsb --lat`/`--lon`/`--show-all`/`--count`/`--interval`**: All standard CLI options matching viewadsb behavior.
- **README.readsb.md**: Dedicated documentation for the core decoder binary.
- **README.beast-client.md**: Dedicated documentation for the beast-client CLI tool.

### Fixed
- **Mode-S ME field extraction off-by-one**: `decode_df17_18` extracted the ME field from `msg[5..12]` instead of `msg[4..11]`, shifting all ME byte indices by one. This caused altitude, velocity, target state, and aircraft status fields to decode from wrong byte positions. CPR lat/lon happened to produce plausible-looking values despite the shift. Fix: changed all three ME/MB/MV copies to `msg[4..11]` in `decode_df17_18`, `decode_df20`, and `decode_df31`. `src/modes/parser.rs`

## [0.9.6] - 2026-05-27

### Fixed
- **Multi-pass demodulation silently dropped all correctable CRC messages**: `syndrome != 0 { continue }` discarded every 1-bit and 2-bit correctable frame that `score_modes_message()` had already scored (700–900 for known-ICAO). Now calls `CrcFixEngine::diagnose(syndrome)` and `CrcFixEngine::fix()` for all repairable syndromes, re-computing CRC after correction. `src/demod/demod_2400.rs`
- **Signal subtraction used fixed constants regardless of signal strength**: `PREAMBLE_HIGH=300` / `QI_HIGH=250` over-subtracted weak signals (<100) to zero and under-subtracted strong signals. Now scales by `signal / 200.0` (0.25×–2.0× range), preserving downstream detectability. `src/demod/signal_subtraction.rs`
- **Histogram ignored samples >255**: silently skipped all values in `compute_histogram()`, biasing the noise floor downward. Now caps at 255 and counts them as full-strength. `src/demod/noise_floor.rs`
- **Message advancement was 28 samples instead of 269**: `msg.len() * 2` re-scanned 89% of the same message samples, causing duplicate detections and wasting subtraction bandwidth. Now uses `MODES_LONG_MSG_SAMPLES` (269). `src/demod/demod_2400.rs`

### Added
- **`Message.corrected` flag for CRC-repaired frames**: `DemodResult` messages now carry `corrected: bool` so the downstream pipeline can distinguish natively-clean from repaired frames. `src/demod/demod_2400.rs`
- **`DiagSnapshot.bitfix` counter**: diagnostic output (`READSB_DIAGNOSTIC=1`) reports corrected-message count alongside `crc_ok`/`crc_fail`, making bitfix rate visible per reporting interval. `src/main.rs`
- **Per-pass diagnostics in multi-pass demod**: `READSB_DIAGNOSTIC=1` emits `DIAG PASS N: threshold=T detected=D kept=K corrected=C` for each pass, enabling direct tuning of `--multi-pass-margin`. `src/demod/demod_2400.rs`

### Changed
- **Main loop now accepts CRC-corrected messages for tracking and console output**: `if result.crc_ok || result.corrected` feeds both clean and repaired frames into tracker, ICAO filter, console outputter, and stats accumulator (`crc_corrected` counter). Beast/Hex output already included all frames regardless of CRC status. `src/main.rs`


## [0.9.5] - 2026-05-27

### Added
- **`--ringbuf-size` / `READSB_RINGBUF_SIZE`**: RTL_TCP ring buffer capacity in bytes
  (default 4MB, ~830ms at 2.4MSPS). Reduces 75% TCP overflow miss rate by decoupling
  the SDR reader from the demodulator's blocking time. `src/config.rs`, `src/sdr/ringbuf.rs`
- **`src/sdr/ringbuf.rs`**: Ring buffer module using `tokio::sync::mpsc::bounded(16)`
  backpressure. A background reader task pushes 262KB chunks; main loop pops at its own
  pace. Capacity is pre-allocated at startup to avoid runtime allocations.
- **RTL_TCP split-stream reader**: `RtlTcpClient` now uses `TcpStream::into_split()`
  to decouple read/write ownership. The write half is retained for `send_command`,
  the read half is passed to a spawned background task that drives the ring buffer.
- **`--multi-pass` / `READSB_MULTI_PASS`**: Enable multi-pass demodulation (default
  `true`). Subtraction of CRC-OK messages from the magnitude buffer between passes,
  recovering weak signals buried under stronger overlapping Mode-S bursts.
- **`--multi-pass-margin` / `READSB_MULTI_PASS_MARGIN`**: Threshold reduction factor
  per pass (default 0.8). Pass N threshold = `noise_floor * 1.5 * 0.8^N`, capped at
  a minimum of `noise_floor * 1.5`. `src/config.rs`
- **`src/demod/signal_subtraction.rs`**: `DecodedMessage::subtract_from()` reconstructs
  the decoded Mode-S IQ pattern via phase/amplitude estimation and subtracts it from
  the magnitude buffer. Exports `MODES_LONG_MSG_SAMPLES` (269), `MODES_LONG_MSG_BYTES`
  (14), `MODES_SHORT_MSG_BYTES` (7) as `pub`.
- **`src/demod/noise_floor.rs`**: O(n) 256-bin histogram noise estimation and
  `adaptive_threshold()` for multi-pass threshold computation.

### Changed
- **`demodulate2400()` return type**: now returns `Vec<(Vec<u8>, f64, usize)>` — the
  third element is the `preamble_pos` (sample index of the detected preamble peak).
- **`demodulate2400_multi_pass()`**: new public function implementing up to 4 passes.
  Pass 0 uses the normal threshold; subsequent passes clone the mag buffer and
  re-demodulate with `adaptive_threshold()`. CRC-OK messages from each pass are
  subtracted before the next pass.
- **`DemodConfig` extended**: gains `multi_pass: bool` and `multi_pass_margin: f64`.
  `src/demod/demod_2400.rs`, `src/main.rs`

## [0.9.4] - 2026-05-23

### Added
- **RTL_TCP overlap buffer**: 300-sample tail-overlap prepended to each SDR read,
  recovering messages that straddle buffer boundaries. Byte width is format-derived
  (2 for U8, 4 for SC16Q11, 8 for F32). SC16Q11M 12-byte header stripped once
  before main loop. `src/main.rs`
- **Smaller RTL_TCP reads**: Read target dynamically set to 262KB (matching USB's
  ~55ms chunks) for RTL_TCP vs 4.8MB for USB/file. Configurable via `READSB_TCP_CHUNK`
  env var. `src/main.rs`
- **`READSB_DIAGNOSTIC=1`**: Per-iteration counters (bytes, samples, preamble
  candidates, messages, CRC ok/fail, reads/sec) logged to stderr every 5 seconds.
  `src/main.rs`
- **Overlap assembly tests**: 6 tests covering all formats, edge cases (n=0,
  n < overlap, n > overlap), magnitude conversion, SC16Q11 header skip.
  `tests/overlap_compat.rs`

### Changed
- `MagBufStats::preamble_candidates` — now populated by counting `pre_found` hits
  in `demodulate2400` (was dead code returning all zeros). `src/demod/demod_2400.rs`
- `SdrType` derives `Clone` for use in read_target computation before `sdr.open()`.
  `src/sdr/manager.rs`

## [0.9.3] - 2026-05-23

### Fixed
- **Short frames can't correct CRC errors**: `CrcFixEngine` was only built for
  112-bit messages. Single-bit errors in DF0/4/5/11 (56-bit) frames were never
  corrected, causing missed altitude/squawk/ICAO updates. Now creates both 56
  and 112-bit engines, selecting by `msgbits` at each call site.
  `src/main.rs`
- **Beast TCP input parser uses wrong frame marker**: `client.rs` parsed Beast
  frames with `0x10` marker and wrong offsets, silently dropping all inbound
  Beast data. Now uses `parse_beast_frame` from `beast.rs` which correctly
  handles `0x1a` markers, 9-byte headers, and byte-stuffing.
  `src/net/client.rs`
- **RTL-TCP single `read()` returns partial data**: TCP `stream.read(buf)`
  can return fewer bytes than the buffer size. Now loops until buffer is full
  or EOF, matching readsb-C's read-all behavior.
  `src/sdr/rtl_tcp.rs`

## [0.9.2] - 2026-05-22

### Fixed
- **Unknown-ICAO frames discarded from demod**: `score_modes_message` returned -1
  for DF0/4/5/16/20/21 with clean CRC and unknown ICAO. Now returns 700 (passes
  through to output). `src/demod/demod_2400.rs`
- **CRC_FAIL frames discarded from demod**: All DFs with uncorrectable CRC errors
  returned -2, dropping valid-but-corrupt frames. Now returns 100 for low-scored
  pass-through to CRC_FAIL output. `src/demod/demod_2400.rs`
- **ICAO filter never populated**: `icao_filter_add()` was defined but never called
  from the main pipeline. Known-aircraft scoring bonuses never applied. Now
  populates filter on every CRC-OK message with a nonzero address.
  `src/main.rs`
- **SDR read errors abort the program**: Transient RTL-SDR errors caused `break`
  and shutdown. Now logs warning, sleeps 100ms, and retries.
  `src/main.rs`

## [0.9.1] - 2026-05-22

### Fixed
- **RTL-SDR "Resource busy" on Linux**: `rtl-sdr-rs` v0.3.1 never detaches the
  kernel driver before `claim_interface()`, returning `LIBUSB_ERROR_BUSY` when
  `dvb_usb_rtl28xxu` is bound. readsb-rs now pre-emptively detaches the kernel
  driver via `rusb` before opening the device. `src/sdr/rtlsdr.rs`

## [0.9.0] - 2026-05-22

### Fixed
- **CRC_FAIL frames dropped from Beast/Hex output**: Output pipeline gated on
  `result.crc_ok`, silently dropping all CRC_FAIL frames. Beast and Hex encoders
  now receive all parser outputs (CRC_OK and CRC_FAIL). Tracker and console
  output still gate on crc_ok. `src/main.rs`
- **AP short frames (DF0/4/5/16/20/21) rejected on CRC error**: `score_modes_message`
  returned `-2` immediately for non-zero syndrome on AP frames, with no CRC
  correction attempt. Added `engine.diagnose()` call for single-bit CRC repair.
  `src/demod/demod_2400.rs`
- **DF11 all-call with interrogator ID rejected on valid CRC**: DF11 frames with
  IID != 0 and syndrome == 0 fell through to `return -2`. Added `return 500/700`
  for valid-CRC DF11 with non-zero IID. `src/demod/demod_2400.rs`
- **Live comparison collected windows sequentially**: `diff_live` called
  `collect_window` for left, then right — covering disjoint time periods (zero
  matches). Now spawns parallel threads for same-window collection.
  `src/bin/beast-client.rs`

### Added
- **`--output <path>` flag**: Redirects compare diff output to a file.
  `src/bin/beast-client.rs`
- **Matched-varying counter**: DF17/DF18/DF19 with differing payloads are counted
  as matched-varying (silent) instead of diff (printed). Summary line:
  `Matched: N, Matched-varying: M, Diff: D, Left-only: L, Right-only: R`.
  `src/bin/beast-client.rs`
- **Progress ticker**: Live window collection shows elapsed time on stderr, with
  10-second markers. `src/bin/beast-client.rs`

### Changed
- Version 0.8.0 → 0.9.0

## [0.8.0] - 2026-05-21

### Added
- **Full `demod_2400.rs` rewrite matching readsb-C**: 8-phase implementation
  covering sample rate update, slice coefficients, ICAO filter, `slice_byte()`,
  preamble detection, tiered scoring, 5-phase decode, AGC loop, and CLI flags.
  `src/demod/demod_2400.rs`
- **Global ICAO filter**: 4096-bucket `LazyLock<Mutex<IcaoFilterInner>>` with
  `icao_filter_add()`, `icao_filter_test()`, `clear_filter()`.
  `src/demod/icao_filter.rs`
- **`DemodConfig`/`DemodResult`/`Message`/`MagBufStats` structs** for
  configurable demodulation pipeline. `src/demod/demod_2400.rs`
- **`demodulate2400_v2()` public API**: takes `DemodConfig`, returns
  `DemodResult` with messages + auto-gain stats. `src/demod/demod_2400.rs`
- **`--agc` and `--preamble-threshold` CLI flags**: `src/config.rs`
- **29 integration tests** for demodulator, ICAO filter, slice functions,
  preamble, scoring, and structs. `tests/demod_compat.rs`

### Changed
- Sample rate 2.0 MHz → 2.4 MHz in `rtl_tcp.rs` and `rtlsdr.rs`
- `MODES_LONG_MSG_SAMPLES` 224→269, `MODES_SHORT_MSG_SAMPLES` 112→135
- Slice coefficients corrected to match readsb-C exactly (all 5 phases)
- `slice_phase4` now takes &[u16] with len ≥ 4 (was 3)
- `encode_beast_output` now takes `(&[u8], f64, u64)` — 3 arguments with
  monotonic microsecond timestamp
- `CrcFixEngine::diagnose(syndrome) -> Option<ErrorInfo>` for inline CRC
  repair in `score_modes_message`
- `decode_frames` dispatches CRC engines by `msgbits` (56 vs 112)
- `decode_message()` removed (replaced by `score_phase` + `slice_byte`)
- `score_modes_message` scoring: -2 (invalid), -1 (DF unknown), 350–1800 tiered
- Integration test `test_synthetic_fixture_pipeline` replaced with
  `test_binary_processes_fixture` smoke test

### Fixed
- 17 clippy lints in test files (manual_is_multiple_of, drop_non_drop,
  field_reassign_with_default, redundant pattern matching, needless_index,
  needless_borrow, literal_out_of_range)

## [0.7.0] - 2026-05-21

### Fixed
- **Beast output format**: Switched from MLAT binary (`0x10 0x02`/`0x10 0x03`
  wrapped) to standard Beast protocol (`0x1a` escape byte, `0x32`/`0x33` frame
  types, RSSI byte, `0x1a` byte-stuffing). Downstream clients (ultrafeeder,
  tar1090) can now parse frames. `src/net/protocols/beast.rs`
- **All ports shared same output**: Single `message_tx` broadcast channel fed
  pre-encoded Beast data to every port (RI/BO/SBS). Replaced with three
  independent channels (`beast_tx`, `hex_tx`, `sbs_tx`) so each port gets
  its correct format. `src/net/server.rs`, `src/net/client.rs`

### Changed
- `encode_beast_output` signature from `(&DecodedMessage) -> Vec<u8>` to
  `(&[u8], f64) -> Vec<u8>`, removing the `DecodedMessage` wrapper
- `ClientConnection::handle()` takes `broadcast::Receiver<Vec<u8>>` instead
  of `broadcast::Receiver<DecodedMessage>` for outbound data
- `NetworkServer::new()` returns 4-tuple (server + 3 format-specific receivers)

### Added
- **Hex output encoder**: `encode_hex_output(&[u8]) -> Vec<u8>` generates
  AVR-compatible `*<hex>;\n` lines. Wired to port 30001.
- **SBS output encoder**: `encode_sbs_aircraft(&Aircraft, i64) -> Vec<u8>`
  generates Basestation CSV lines for tracked aircraft every second.
  Emits MSG,7 (ICAO), MSG,8 (signal) always; MSG,5 (altitude), MSG,1
  (callsign), MSG,3 (position), MSG,4 (velocity) when data is valid.
  `src/net/protocols/sbs.rs`
- **`escape_beast()` helper**: `src/net/protocols/beast.rs` — byte-stuffing
  for standard Beast `0x1a` payload escaping
- **11 new tests** across Beast/Hex/SBS encoders and server architecture
- **Periodic SBS task**: 1-second interval iterates tracked aircraft and
  sends SBS updates on `sbs_tx` channel

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
