# Changelog

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
