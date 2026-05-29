# readsb-rs

**A Rust rewrite of [readsb](https://github.com/wiedehopf/readsb) — Mode-S/ADSB/TIS message decoder**

readsb-rs is a from-scratch Rust implementation of the popular `readsb` ADS-B receiver, originally written in C by Mictronics with contributions from wiedehopf and others. This project reimplements the full signal processing pipeline — from raw I/Q samples to decoded aircraft state — in safe, idiomatic Rust.

## Status

This is an **active rewrite in progress** covering the full demodulation, decoding, and network serving pipeline.

| Module | Status | Tests |
|--------|--------|-------|
| CRC-24 + error correction | ✅ Complete | 8 |
| Compact Position Reporting | ✅ Complete | 14 |
| Mode S message parsing | ✅ Complete | 6 |
| Comm-B / Mode A/C / AIS | ✅ Complete | 8 |
| Aircraft tracking + registry | ✅ Complete | 28 |
| I/Q format conversion | ✅ Complete | 6 |
| 2.4MHz Manchester demodulator | ✅ Complete | 29 |
| SDR hardware abstraction | ✅ Complete (FFI stub) | 6 |
| TCP server + protocol parsers | ✅ Complete | 26 |
| Beast/Hex/SBS encoders | ✅ Complete | 3 |
| JSON / globe / heatmap output | ✅ Complete | 4 |
| CLI configuration | ✅ Complete | 3 |
| Async main loop | ✅ Complete | 5 |
| Console output (tiered verbosity) | ✅ Complete | 30 |
| Runtime stats collection | ✅ Complete | 2 |
| beast-client CLI tool | ✅ Complete | 43 |
| viewsb interactive table | ✅ Complete | 24 |
| Docker + CI | ✅ Complete | — |

**Total: 244 tests, all passing**

## Tools

This repository provides two binaries:

- **[`readsb`](README.readsb.md)** — The core decoder. Reads I/Q samples from an SDR or file, demodulates Mode-S messages, tracks aircraft, and serves data over TCP.
- **[`beast-client`](README.beast-client.md)** — Companion CLI for Beast-format data. Subcommands: `decode`, `hex`, `live`, `record`, `play`, `compare`, `viewsb`.

## Quick Start

```bash
cargo build --release

# Live SDR
./target/release/readsb --device-type rtlsdr --gain 49.6

# File replay
./target/release/readsb --ifile samples.bin --iformat sc16q11

# Network only (no SDR)
./target/release/readsb --net --net-bo-port 30005 --net-ri-port 30002

# Interactive aircraft table (from a Beast source)
./target/release/beast-client viewsb --host 127.0.0.1 --port 30005

# One-shot JSON snapshot
./target/release/beast-client viewsb --host 127.0.0.1 --port 30005 --json --count 1
```

See [README.readsb.md](README.readsb.md) and [README.beast-client.md](README.beast-client.md) for detailed documentation.

## Architecture

```
I/Q samples → convert → demodulate → parse → track → output
                │           │         │       │       │
             SC16Q11     2.4MHz    Mode S   Per-   JSON/TCP
             F32/U8     Manchester  CRC+DF   aircraft
```

## Cross-Validation

CRC and CPR algorithms are cross-validated against the original C reference:
```bash
# Run C reference tests
cd /CODE/readsb && make crctests && ./crctests
cd /CODE/readsb && make cprtests && ./cprtests

# Run Rust tests
cd readsb-rs && cargo test
```

## Credits

This project is a Rust rewrite of **readsb** by Mictronics, wiedehopf, and contributors. The original C codebase is at [github.com/wiedehopf/readsb](https://github.com/wiedehopf/readsb) and is licensed under GPL v3.

- **Original readsb** — Mictronics, wiedehopf, and the ADS-B decoding community
- **dump1090** — The foundational Mode S decoder by antirez
- **FlightAware** — dump1090-fa improvements and protocol definitions

## License

GNU General Public License v3.0 or later. See [COPYING](COPYING) for details.

This project is a clean-room Rust implementation of the algorithms described in the original C codebase. The original C code is not included — only the signal processing algorithms and protocol specifications are reimplemented.
