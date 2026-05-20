# readsb-rs

**A Rust rewrite of [readsb](https://github.com/wiedehopf/readsb) — Mode-S/ADSB/TIS message decoder**

readsb-rs is a from-scratch Rust implementation of the popular `readsb` ADS-B receiver, originally written in C by Mictronics with contributions from wiedehopf and others. This project reimplements the full signal processing pipeline — from raw I/Q samples to decoded aircraft state — in safe, idiomatic Rust.

## Status

This is an **active rewrite in progress** covering the full demodulation, decoding, and network serving pipeline.

| Module | Status | Tests |
|--------|--------|-------|
| CRC-24 + error correction | ✅ Complete | 8 |
| Compact Position Reporting | ✅ Complete | 14 |
| Mode S message parsing | ✅ Complete | 9 |
| Comm-B / Mode A/C / AIS | ✅ Complete | 6 |
| Aircraft tracking + registry | ✅ Complete | 12 |
| I/Q format conversion | ✅ Complete | 3 |
| 2.4MHz Manchester demodulator | ✅ Complete | 3 |
| SDR hardware abstraction | ✅ Complete (FFI stub) | — |
| TCP server + protocol parsers | ✅ Complete | 4 |
| JSON / globe / heatmap output | ✅ Complete | — |
| CLI configuration | ✅ Complete | — |
| Async main loop | ✅ Complete | — |
| Docker + CI | ✅ Complete | — |

**Total: 53 tests, all passing**

## Architecture

```
I/Q samples → convert → demodulate → parse → track → output
                │           │         │       │       │
             SC16Q11     2.4MHz    Mode S   Per-   JSON/TCP
             F32/U8     Manchester  CRC+DF   aircraft
```

The pipeline is fully async (tokio). The main loop reads I/Q samples from an SDR device, converts them to magnitude values, demodulates Mode S messages, parses them, updates per-aircraft tracking state, and serves the data over TCP.

## Quick Start

```bash
cargo build --release

# Live SDR
./target/release/readsb --device-type rtlsdr --gain 49.6

# File replay
./target/release/readsb --ifile samples.bin --iformat sc16q11

# Network only (no SDR)
./target/release/readsb --net --net-bo-port 30005 --net-ri-port 30002
```

## Key Differences from C readsb

| Aspect | C readsb | readsb-rs |
|--------|----------|-----------|
| Safety | Manual memory management | Compile-time memory safety |
| Concurrency | Raw pthreads + mutexes | Tokio async + RwLock |
| CRC | Table-driven, data + XOR | Bit-serial, verified bit-exact |
| JSON | Manual string building | serde derive macros |
| Binary formats | Packed structs + memcpy | `#[repr(C)]` safe transmutes |
| Build | Makefile | Cargo (cross-compile friendly) |
| SDR drivers | Direct FFI to librtlsdr | Async trait (FFI stub) |
| Main loop | `while(1)` + epoll | `tokio::select!` |

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
