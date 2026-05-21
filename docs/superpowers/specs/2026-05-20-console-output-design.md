# Console Output Design — readsb-rs

## Motivation

The original READSB C project produced per-message console output with decoded fields, CRC status, RSSI, and timing — invaluable for debugging antenna placement, signal quality, and decoder behavior. The Rust rewrite currently outputs only a terse status line every 100 iterations (`[FIRST 1 bytes=... decodes=... aircraft=...]`), plus periodic `info!` messages for lifecycle events. For Docker deployments with flat logs, rich console output is the primary troubleshooting interface.

## Control Mechanism

- **Startup level:** `CONSOLE_LEVEL` env var, values: `low` | `medium` | `high` | `max`. Default `low`.
- **Runtime level change:** SIGUSR1 cycles forward (low→medium→high→max→low), SIGUSR2 cycles backward.
- **Medium tier interval:** `CONSOLE_INTERVAL` env var, seconds. Default `10`.

## Verbosity Tiers

### Low — Aggregate Stats Line (1 line / 60 seconds)

One unified block printed to stdout once per minute:

```
[readsb] 14:32:01 msgs/s=1845 ac=32 ac1h=47 crc_bad=2.1% bitfix=0.3%
  sig: -12.3 avg / -8.1 max / -31.2 min dBFS  drops=0
  df: DF17=78% DF11=12% DF0=5% DF20=3% DF4=2%
  cpr_ok=89% uptime=4h12m
```

| Metric | Source |
|--------|--------|
| `msgs/s` | Message rate over rolling 60s window |
| `ac` | Currently tracked aircraft (not stale) |
| `ac1h` | Unique aircraft seen in last hour |
| `crc_bad` | % of frames failing CRC check (including after bit fix) |
| `bitfix` | % of frames where single-bit correction succeeded |
| `sig avg/max/min` | RSSI statistics over the interval |
| `drops` | SDR sample underrun count since last stats |
| `df` | Message type distribution as percentages |
| `cpr_ok` | % of CPR position decode attempts succeeding |
| `uptime` | Time since process start + start timestamp |

### Medium — Per-Aircraft Change Summary (every N seconds, default 10)

One line per aircraft active in the last interval (using the tracker's stale timeout, default ~60s). Only fields that changed since the last report are shown. Change indicators: `▲` (increased), `▼` (decreased), `▸` (new value, i.e. first seen in interval). If a field becomes invalid (e.g. position decode lost), it is shown as `field:---`.

```
[A43EA2] alt:27600▲ gs:378 trk:352 callsign:ASA1390  sig:-10.1
[A5C899] alt:14300▸ pos:46.38,-122.31  sig:-12.2▼
```

Tracked fields and their change-detection: hex address (always shown), baro altitude, geom altitude, groundspeed, track, vertical rate, callsign, latitude, longitude, signal level, squawk, category, air/ground state. A timestamp prefix is added if `--log-timestamps` is enabled.

### High — Per-Message Decoded (no DF11 / no empty frames)

Compact one-block-per-message, printed as each message is decoded. Excludes DF11 (All-Call), DF0/4/16 with no extracted data, and other messages where only the ICAO address is known.

```
[A43EA2] DF17 vel  gs:378.5 trk:352.3 alt:27675 rate:-2240 nacv:1  sig:-12.1dBFS
[A6C311] DF17 id   callsign:ASA1390 cat:A3  sig:-24.2dBFS
[A324B0] DF17 pos  alt:27000 pos:45.34,-121.61 cpr:odd  sig:-18.1dBFS
[A1440C] DF29 st   hdg:348.8 sel_alt:4992  sig:-27.6dBFS
```

Format: `[ICAO] DF<type> <short_desc>  <field>=<val>...  sig:<rssi>dBFS`

### Maximum — Every CRC-Passing Message (including DF11 / empty)

Same block format as High, but includes DF11 All-Call, empty Comm-B, and any other message that passes CRC regardless of content:

```
[A6C311] DF11 all-call  sig:-24.2dBFS
[A5C899] DF20 comm-b   sig:-10.1dBFS
```

## Output Channel

Console output goes to **stdout** via `writeln!` to `io::stdout()`, keeping it separate from `tracing`-based structured logging which goes to stderr. This allows `docker logs` to show console output without mixing in tracing noise, while `RUST_LOG` controls diagnostic logging independently.

## Module Structure

```
src/console/
├── mod.rs          — Module root, re-exports public API
├── formatter.rs    — Pure formatting functions (no I/O, no state)
├── state.rs        — ConsoleLevel, Snapshot diff, Level cycling (pure logic)
├── outputter.rs    — Outputter orchestrator (stateful, calls formatter + state)
└── tests.rs        — Integration tests (or inline #[cfg(test)])
```

### `formatter.rs` — Pure formatting functions

Each function takes data and returns a `String`. No I/O, no mutable state. Perfect for TDD.

```
pub fn format_low(stats: &AggregatedStats, uptime: Duration) -> String;
pub fn format_medium(icao: u32, changes: &[FieldChange]) -> String;
pub fn format_high(msg: &ModesMessage) -> Option<String>;   // None = skip (DF11/empty)
pub fn format_max(msg: &ModesMessage) -> Option<String>;    // None = skip (crc failed)
```

### `state.rs` — Pure types and logic

```
#[derive(Clone, Copy, PartialEq)]
pub enum ConsoleLevel { Low, Medium, High, Max }

pub fn cycle_level(current: ConsoleLevel, forward: bool) -> ConsoleLevel;

#[derive(Clone)]
pub struct AircraftSnapshot {
    pub icao: u32,
    pub baro_alt: Option<i32>,
    pub geom_alt: Option<i32>,
    pub gs: Option<f32>,
    pub track: Option<f32>,
    pub baro_rate: Option<i32>,
    pub callsign: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub signal_level: Option<f64>,
    pub squawk: Option<u32>,
    pub category: Option<u8>,
    pub airground: Option<AirGround>,
}

pub enum FieldChange {
    Unchanged,
    New { value: String },
    Increased { value: String, delta: f64 },
    Decreased { value: String, delta: f64 },
    Lost,   // was valid, now invalid
}

pub fn diff_snapshots(prev: &AircraftSnapshot, current: &AircraftSnapshot) -> Vec<(&str, FieldChange)>;

pub fn snapshot_from_message(msg: &ModesMessage) -> Option<AircraftSnapshot>;   // None if ICAO unknown
```

### `outputter.rs` — Stateful orchestrator

```
pub struct Outputter {
    level: ConsoleLevel,
    medium_interval: Duration,
    last_low_report: Instant,
    last_medium_report: Instant,
    last_aircraft_states: HashMap<u32, AircraftSnapshot>,
    start_time: Instant,
    stats_accumulator: StatsAccumulator,
}

impl Outputter {
    pub fn new(level: ConsoleLevel, medium_interval: Duration) -> Self;
    pub fn set_level(&mut self, level: ConsoleLevel);
    pub fn feed(&mut self, msg: &ModesMessage, now: Instant) -> Vec<String>;  // returns lines to print
    pub fn flush_medium(&mut self, now: Instant) -> Vec<String>;
    pub fn flush_low(&mut self, now: Instant) -> Vec<String>;
}
```

`feed()` returns owned `Vec<String>` rather than writing directly — makes the orchestrator testable without capturing stdout.

## Integration points in `main.rs`

1. **After startup config is parsed:** create `Outputter` from env vars
2. **Signal handler setup:** register SIGUSR1/SIGUSR2 to atomically swap `CURRENT_LEVEL`
3. **In the main loop, after `parse_modes_message`:** call `outputter.feed(&result.message, now)` and print returned lines
4. **Stats timer:** the existing 60s stats info! call is replaced by Outputter's low-level reporting
5. **Shutdown:** Outputter prints a final low-level stats summary line

## Output ordering

Output from different tiers (low aggregate vs medium per-aircraft) serializes through the same `Outputter` write path. When both fire in the same tick, low (aggregate) is printed first, followed by medium (per-aircraft). Within a tier, output is written atomically line-by-line.

## Signal handling

```
static CURRENT_LEVEL: AtomicU8 = ...;
// SIGUSR1: cycle forward
// SIGUSR2: cycle backward
// Outputter reads CURRENT_LEVEL on each feed() call
```

The cycling logic itself is a pure function `cycle_level(current, forward) -> ConsoleLevel` defined in `state.rs`, tested independently from signal delivery.

## Testing Strategy

Every component is designed for test-first development. The pure functions in `formatter.rs` and `state.rs` require no mocks, no I/O, no setup.

### Unit tests — `formatter.rs` (at least 8 tests)

1. `test_format_low_basic` — feed known `AggregatedStats` + uptime, expect exact string output including all fields
2. `test_format_low_zero_stats` — all stats at zero, verify no division-by-zero, output has zeros
3. `test_format_low_with_drops` — verify `drops=N` appears when drops > 0
4. `test_format_medium_change_indicators` — construct `[FieldChange::Increased {..}]`, verify `▲` present in output
5. `test_format_medium_unchanged` — empty changes vec, verify ICAO alone is printed
6. `test_format_medium_lost_field` — `FieldChange::Lost` produces `field:---`
7. `test_format_high_airborne_velocity` — synthetic `ModesMessage` (DF17, velocity type), verify output contains `gs:`, `trk:`, `alt:`
8. `test_format_high_skips_df11` — DF11 message returns `None`
9. `test_format_high_position` — DF17 position message, verify `pos:` in output
10. `test_format_high_callsign` — DF17 ID message with callsign, verify `callsign:` in output
11. `test_format_max_includes_df11` — DF11 message returns `Some(...)` with `all-call`

### Unit tests — `state.rs` (at least 5 tests)

1. `test_cycle_level_forward` — start at Low, cycle forward 5 times, expect Low→Medium→High→Max→Low
2. `test_cycle_level_backward` — start at Low, cycle backward, expect Low→Max→High→Medium→Low
3. `test_diff_snapshots_all_unchanged` — identical snapshots return all `Unchanged`
4. `test_diff_snapshots_alt_changed` — prev=10000, current=10500 → `Increased { delta: 500 }`
5. `test_diff_snapshots_field_lost` — prev has altitude, current has None → `Lost`
6. `test_snapshot_from_message` — populate a `ModesMessage` with known values, verify `AircraftSnapshot` fields match

### Integration tests — via `feed()` (at least 3 tests)

1. `test_outputter_low_tier` — create `Outputter` at Low level, feed 100 messages with synthetic timestamps advancing 1s each, verify `flush_low()` returns exactly 1 string (the aggregate line)
2. `test_outputter_high_tier` — create `Outputter` at High level, feed a DF17 velocity message, verify `feed()` returns 1 string containing `DF17 vel`
3. `test_outputter_max_tier` — create `Outputter` at Max level, feed a DF11 message, verify output contains `DF11 all-call`
4. `test_outputter_medium_change_detection` — feed same ICAO twice with different altitude, verify medium output includes `▲`

### Test fixtures

Tests construct `ModesMessage` directly via `ModesMessage::default()` + field assignment, matching the existing pattern in `tests/modes_compat.rs`. No fixtures files or data files needed for console tests.

### Running tests

```
cargo test --lib console::    # unit tests only
cargo test console_output     # all console tests
```

All tests must:
- Fail in RED phase before implementation (proving the test detects missing code)
- Pass in GREEN phase after minimal implementation
- Remain green through REFACTOR phase

## StatsAccumulator

A new lightweight accumulator separate from the existing `Stats` struct, designed for rolling-window stats:

```
pub struct StatsAccumulator {
    // Rolling 60s window
    message_timestamps: VecDeque<Instant>,
    messages_by_type: [u32; 32],         // indexed by DF type
    crc_fail_count: u32,
    bitfix_count: u32,
    cpr_global_ok: u32,
    cpr_global_bad: u32,
    cpr_local_ok: u32,
    signal_readings: VecDeque<(Instant, f64)>,  // for min/max/avg
    drop_count: u64,
    unique_aircraft_last_hour: HashSet<u32>,
    currently_tracked: AtomicU32,         // shared with tracker
}
```

This is intentionally separate from the existing `crate::stats::Stats` (which is a cumulative counter used by the network output). The `StatsAccumulator` focuses on display-oriented rolling window stats.

## What is NOT in scope

- Aircraft type / registration lookup (requires an external database)
- JSON or structured machine-readable output
- Colorized terminal output (not useful in Docker flat logs)
- Web UI — this is purely console output

## Future Considerations

- `CONSOLE_LEVEL` could be aliased to a `-v` / `--verbose` CLI flag
- Aircraft type lookup (via basestation.sqb or OpenSky API) could add a `type` field to medium/high output
- Stats could be exposed via a simple HTTP endpoint for prometheus scraping
