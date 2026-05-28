# readsb — Mode-S/ADSB/TIS Decoder

**The core decoder binary** — decodes raw I/Q samples from an SDR or file into Mode-S messages, tracks aircraft state, and serves data over TCP.

## Pipeline

```
I/Q samples → convert → demodulate → parse → track → output
                │           │         │       │       │
             SC16Q11     2.4MHz    Mode S   Per-   JSON/TCP
             F32/U8     Manchester  CRC+DF   aircraft
```

The pipeline is fully async (tokio). The main loop reads I/Q samples from an SDR device, converts them to magnitude values, demodulates Mode S messages, parses them, updates per-aircraft tracking state, and serves the data over TCP.

## Quick Start

```bash
# Live SDR
readsb --device-type rtlsdr --gain 49.6

# File replay
readsb --ifile samples.bin --iformat sc16q11

# Network only (no SDR)
readsb --net --net-bo-port 30005 --net-ri-port 30002
```

## Options

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
| `READSB_NET` | `--net` | — | Enable network server |
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

Boolean flags (`--net`, `--aggressive`, `--debug-*`, `--quiet`): set env var to any value (e.g. `"yes"`, `"1"`) to enable.

## Console Output

Console output is printed to stdout in four verbosity levels, controlled via environment variable or runtime UNIX signals.

### Verbosity Levels

| Level | `CONSOLE_LEVEL` | Output |
|-------|-----------------|--------|
| **Low** (default) | `low` | One aggregate stats block per minute — msg rate, aircraft count, CRC quality, signal stats, DF distribution, CPR rate, uptime |
| **Medium** | `medium` | Per-aircraft change summary every N seconds — each line shows only fields that changed since the last report |
| **High** | `high` | Compact per-message output for each decoded ADS-B message (skips DF11 All-Call and empty frames) |
| **Max** | `max` | Every CRC-passing message, including DF11/empty frames |

### Runtime level cycling

Send `SIGUSR1` to cycle forward (Low → Medium → High → Max → Low) or `SIGUSR2` to cycle backward:
```
docker kill -s SIGUSR1 <container>
docker kill -s SIGUSR2 <container>
```

## Docker

```
docker pull ghcr.io/dlasher/readsb-rs:latest
docker run --rm --cap-add SYS_RAWIO --device /dev/bus/usb \
  ghcr.io/dlasher/readsb-rs:latest \
  --device-type rtlsdr --gain 49.6
```

### Docker Compose

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
| CLI comparison tool | None | Built-in `beast-client` (decode, compare, viewsb, record, play) |

## License

GNU General Public License v3.0 or later. See [COPYING](COPYING) for details.
