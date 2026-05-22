# readsb-rs — Agent Guide

## Build, test, lint
- `cargo test --all-features` — CI runs this (needs libusb-1.0-0-dev)
- `cargo clippy -- -D warnings` — CI enforces no warnings
- `cargo test --all-features <test_name>` — focused test
- Benchmarks: `cargo bench` (2 criterion benches, `harness = false`)
- No formatter/clippy config — uses default toolchain rules

## Test layout
- **192 total**: 162 integration (`tests/`) + 30 unit (`src/console/`)
- `tests/tracking_compat/` is a directory — contains `test_aircraft.rs`, `test_tracker.rs`, `test_validity.rs` (24 tests total)
- Console module tests live inline in `src/console/*.rs` (not in tests/)

## Architecture
- Single `readsb` crate, no workspace
- `src/lib.rs` re-exports 12 modules: config, sdr, types, crc, cpr, demod, modes, tracking, net, output, stats, console
- `src/main.rs` wires async pipeline: SDR → demod → parse → track → output
- Config: clap derive with `#[arg(env = "READSB_*")]` — all CLI flags settable via env vars
- Console output goes to **stdout** (tracing goes to stderr)

## Releases & Docker
- Binary releases only on `v*` tag push (CI checks `startsWith(github.ref, 'refs/tags/')`)
- Docker images pushed to GHCR on every main push (latest + semver tags)
- `--gain auto` must be skipped — docker-entrypoint.sh handles this

## Console output quirks
- Controlled via `CONSOLE_LEVEL` env var (low/medium/high/max) + SIGUSR1 (forward) / SIGUSR2 (backward)
- `CONSOLE_INTERVAL` sets medium tier period (default 10s)
- Levels: Low (60s aggregate) → Medium (per-aircraft changes) → High (per-message, skip DF11) → Max (all CRC-passing)

## Known issues
- `benches/demod_bench.rs:5` — `u8 % 256` is dead code (bench placeholder, harmless)
- Working on `main` branch — no develop branch
