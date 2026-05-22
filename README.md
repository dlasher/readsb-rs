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
| beast-client CLI tool | ✅ Complete | 19 |
| Docker + CI | ✅ Complete | — |

**Total: 192 tests, all passing**

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

## Console Output

Console output is printed to stdout in four verbosity levels, controlled via environment variable or runtime UNIX signals. This mirrors the per-message diagnostic output from the original C readsb.

### Verbosity Levels

| Level | `CONSOLE_LEVEL` | Output |
|-------|-----------------|--------|
| **Low** (default) | `low` | One aggregate stats block per minute — msg rate, aircraft count, CRC quality, signal stats, DF distribution, CPR rate, uptime |
| **Medium** | `medium` | Per-aircraft change summary every N seconds — each line shows only fields that changed since the last report |
| **High** | `high` | Compact per-message output for each decoded ADS-B message (skips DF11 All-Call and empty frames) |
| **Max** | `max` | Every CRC-passing message, including DF11/empty frames — same detail as the original C readsb |

### Runtime level cycling

Send `SIGUSR1` to cycle forward (Low → Medium → High → Max → Low) or `SIGUSR2` to cycle backward:

```bash
docker kill -s SIGUSR1 <container>   # increase verbosity
docker kill -s SIGUSR2 <container>   # decrease verbosity
```

### Medium tier interval

`CONSOLE_INTERVAL=10` sets the seconds between per-aircraft summary updates (default `10`).

### Examples

**Low (1 line/minute):**
```
[readsb] msgs/s=1845 ac=32 ac1h=47 crc_bad=2.1% bitfix=0.3%
  sig: -12.3 avg / -8.1 max / -31.2 min dBFS  drops=0
  df: DF17=78% DF11=12% DF0=5%  cpr_ok=89% uptime=4h12m
```

**Medium (per-aircraft changes):**
```
[A43EA2] alt:27600▲ gs:378 trk:352 callsign:ASA1390  sig:-10.1
[A5C899] alt:14300▸ pos:46.38,-122.31  sig:-12.2▼
```

**High (per decoded message):**
```
[A43EA2] DF17 vel  gs:378.5 trk:352.3 alt:27675 rate:-2240  sig:-12.1dBFS
[A6C311] DF17 id   callsign:ASA1390 cat:A3  sig:-24.2dBFS
[A324B0] DF17 pos  alt:27000 pos:45.34,-121.61  sig:-18.1dBFS
```

## beast-client

`beast-client` is a companion CLI tool included with readsb-rs for working with Beast-format ADS-B data. It connects to TCP Beast sources (or plays back recorded files) and provides subcommands for inspection, comparison, and live monitoring.

### Commands

| Command | Description |
|---------|-------------|
| `decode` | Connect to a Beast source and print decoded Mode-S messages with timestamp, DF type, and ICAO address |
| `hex` | Connect to a Beast source and print raw hex-encoded frames (AVR format) |
| `live` | Connect to a Beast source and continuously print decoded messages with DF type and ICAO |
| `record` | Connect to a Beast source and save raw frames to a binary record file |
| `play` | Play back a record file in hex or decoded mode |
| `compare` | Side-by-side diff of two Beast sources (file/file, file/live, live/file, or live/live) |

### Compare (live/live)

```bash
beast-client compare --host1 10.4.10.155 --port1 40005 \
                     --host2 127.0.0.1 --port2 30005 \
                     --window 30 --duration 180 --output diff.txt
```

Collects frames simultaneously from both hosts over the same wall-clock window (parallel threads). Output shows ICAOs unique to each source (`<` left-only, `>` right-only), differing payloads (`|`), and a summary line per window:

```
=== Window 0-30s (L: 1000, R: 275) ===
  A43EA2 DF17: 8DA43EA258990A0CCE3820D9EA38 <
                              >  AC16BB DF18: 93AC16BB99210E1156F30C536CF9
--- Matched: 11, Matched-varying: 13, Diff: 3, Left-only: 251, Right-only: 207 ---
```

- **Matched**: identical ICAO+DF pairs (same payload)
- **Matched-varying**: DF17/18/19 frames with time-varying payloads (expected, silently counted)
- **Diff**: differing short frames (potential decode bug)
- **Left-only / Right-only**: ICAO+DF pairs unique to one source

Progress is shown on stderr with tick markers every 10 seconds.

### Compare (file/live)

```bash
beast-client compare --ref1 recorded_traffic.bin \
                     --host2 127.0.0.1 --port2 30005 \
                     --window 30 --duration 60
```

Useful for regression testing — compare a known-good recording against a live decoder.

### Record & Play

```bash
# Record 30 seconds of live Beast traffic
beast-client record --host 127.0.0.1 --port 30005 --output traffic.bin
# Play it back
beast-client play traffic.bin --hex
beast-client play traffic.bin --decode
```

## Docker

```bash
docker pull ghcr.io/dlasher/readsb-rs:latest
docker run --rm --cap-add SYS_RAWIO --device /dev/bus/usb \
  ghcr.io/dlasher/readsb-rs:latest \
  --device-type rtlsdr --gain 49.6
```

### Docker Compose / Swarm

```yaml
services:
  adsb-decoder:
    image: ghcr.io/dlasher/readsb-rs:latest
    network_mode: host
    devices:
      - /dev/bus/usb
    environment:
      READSB_DEVICE_TYPE: "rtlsdr"
      READSB_DEVICE: "rtl_tcp:172.21.0.1:1234"
      READSB_NET: "yes"
      READSB_NET_BO_PORT: "30005"
      READSB_GAIN: "49.6"
      READSB_PPM: "0"
      READSB_LAT: "${FEEDER_LAT}"
      READSB_LON: "${FEEDER_LONG}"
```

### Environment Variables

All CLI flags can be set via `READSB_*` environment variables:

| Variable | CLI flag | Default | Description |
|----------|----------|---------|-------------|
| `READSB_DEVICE_TYPE` | `--device-type` | `rtlsdr` | SDR backend: `rtlsdr`, `rtl_tcp`, or `ifile` |
| `READSB_DEVICE` | `--device` | — | Device spec: USB index, or `rtl_tcp:host:port` |
| `READSB_GAIN` | `--gain` | — | Tuner gain in dB (e.g. `49.6`) |
| `READSB_FREQ` | `--freq` | `1090000000` | Center frequency in Hz |
| `READSB_PPM` | `--ppm` | — | Frequency correction in PPM |
| `READSB_RTLTCP_DIRECT_SAMP` | `--rtltcp-direct-samp` | — | RTL-TCP direct sampling mode |
| `READSB_RTLTCP_OFFSET_TUNE` | `--rtltcp-offset-tune` | — | RTL-TCP offset tuning |
| `READSB_RTLTCP_BIAS_TEE` | `--rtltcp-bias-tee` | — | RTL-TCP bias tee enable |
| `READSB_NET` | `--net` | — | Enable network server (set any value to enable) |
| `READSB_NET_BO_PORT` | `--net-bo-port` | `30005` | Beast output TCP port |
| `READSB_NET_RI_PORT` | `--net-ri-port` | `30002` | Raw input TCP port |
| `READSB_NET_SBS_PORT` | `--net-sbs-port` | `30003` | SBS-1 output TCP port |
| `READSB_NET_BIND_ADDRESS` | `--net-bind-address` | — | Bind address for all network ports |
| `READSB_LAT` | `--lat` | — | Receiver latitude |
| `READSB_LON` | `--lon` | — | Receiver longitude |
| `READSB_MAX_RANGE` | `--max-range` | — | Maximum range in km |
| `READSB_JSON_DIR` | `--json-dir` | — | JSON output directory |
| `READSB_JSON_GLOBE_INDEX` | `--json-globe-index` | — | Write globe index file |
| `READSB_JSON_RELIABLE` | `--json-reliable` | — | JSON write interval |
| `READSB_JSON_TRACE_INTERVAL` | `--json-trace-interval` | — | JSON trace interval |
| `READSB_IFILE` | `--ifile` | — | Input file path |
| `READSB_IFORMAT` | `--iformat` | — | Input format (`CU8`, `SC16`, `CF32`) |
| `READSB_DECODE_THREADS` | `--decode-threads` | `2` | Decode thread count |
| `READSB_AGGRESSIVE` | `--aggressive` | — | Aggressive CRC correction |
| `CONSOLE_LEVEL` | — | `low` | Console verbosity: `low`, `medium`, `high`, `max` |
| `CONSOLE_INTERVAL` | — | `10` | Medium tier update interval in seconds |
| `READSB_DEBUG_NET` | `--debug-net` | — | Network debug logging |
| `READSB_DEBUG_CPR` | `--debug-cpr` | — | CPR debug logging |
| `READSB_DEBUG_GARBAGE` | `--debug-garbage` | — | Garbage detection debug |
| `READSB_DEBUG_API` | `--debug-api` | — | API debug logging |
| `READSB_QUIET` | `--quiet` | — | Suppress status output |

Boolean flags (`--net`, `--aggressive`, `--debug-*`, `--quiet`): set the env var to any value (e.g. `"yes"`, `"1"`, `"true"`) to enable.

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
| Console output | Verbose per-message (always on) | Tiered verbosity with runtime level cycling |
| CLI comparison tool | None — requires external tools | Built-in `beast-client compare` (file/file, file/live, live/live) |

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
