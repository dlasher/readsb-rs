# beast-client — Beast Protocol CLI Tool

**A companion CLI tool** for working with Beast-format ADS-B data. Connects to TCP Beast sources (or plays back recorded files) and provides subcommands for inspection, comparison, and live monitoring.

## Commands

| Command | Description |
|---------|-------------|
| `decode` | Connect to a Beast source and print decoded Mode-S messages with timestamp, DF type, and ICAO address |
| `hex` | Connect to a Beast source and print raw hex-encoded frames (AVR format) |
| `live` | Connect to a Beast source and continuously print decoded messages with DF type and ICAO |
| `record` | Connect to a Beast source and save raw frames to a binary record file |
| `play` | Play back a record file in hex or decoded mode |
| `compare` | Side-by-side diff of two Beast sources (file/file, file/live, live/file, or live/live) |
| `viewsb` | Interactive terminal table of tracked aircraft — like `viewadsb` from readsb-C |

## viewsb — Interactive Aircraft Table

`beast-client viewsb` connects to a Beast source and displays a live-updating table of all tracked aircraft, similar to the `viewadsb` utility from readsb-C.

### Interactive Mode (default)

When connected to a TTY, viewsb displays a full-screen table that updates every second:

```
 ICAO     Flight        Alt       Speed   Heading   Lat       Lon       Dist     Seen
 A43EA2   ASA1390       27600     378     352       37.771   -122.758   1.2 NM   0.1s
 A5C899                 14300     220     180       38.041   -121.761   85 NM    2.3s
```

- Automatically sorts by distance if `--lat`/`--lon` are provided
- Press `q` or `Ctrl-C` to exit
- Uses crossterm for terminal management (alternate screen, cursor hide, cleanup on exit)

### Non-Interactive Mode

When stdout is not a TTY (piped to a file or another program), viewsb prints the table once and exits:

```
beast-client viewsb --host 127.0.0.1 --port 30005 --count 1
```

Or stream snapshots:
```
beast-client viewsb --host 127.0.0.1 --port 30005 --interval 5
```

### JSON/CSV Output

```bash
# One-shot JSON snapshot
beast-client viewsb --host 127.0.0.1 --port 30005 --json

# Streaming CSV
beast-client viewsb --host 127.0.0.1 --port 30005 --csv --interval 10
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `127.0.0.1` | Beast source host |
| `--port` | `30005` | Beast source port |
| `--lat`, `--lon` | — | Receiver position (for distance column and distance sort) |
| `--sort` | `none` | Sort column: `none`, `icao`, `flight`, `alt`, `speed`, `heading`, `distance`, `seen` |
| `--metric` | — | Use metric units (m, km/h, km) instead of imperial (ft, kt, NM) |
| `--count` | — | Snapshot count in non-interactive mode (default: unlimited) |
| `--interval` | `1` | Seconds between snapshots |
| `--show-all` | — | Show all aircraft, including those without position |
| `--json` | — | Output JSON lines |
| `--csv` | — | Output CSV lines |

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

### Record & Play

```bash
# Record 30 seconds of live Beast traffic
beast-client record --host 127.0.0.1 --port 30005 --output traffic.bin

# Play it back
beast-client play traffic.bin --hex
beast-client play traffic.bin --decode
```
