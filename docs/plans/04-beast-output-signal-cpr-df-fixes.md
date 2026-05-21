# Plan: Beast Output, Signal Pipeline, CPR Stats, DF Windowing, Position Wiring

## Problem Summary

Downstream clients connected to the Beast output port see zero aircraft.
Root cause: Beast output framing uses wrong MLAT sync header and message type byte.
Five additional issues discovered during investigation.

---

## Already Fixed (Cycle 0)

**Beast output framing** — `src/net/protocols/beast.rs` + `src/net/client.rs` + `tests/net_compat.rs`

| Bug | Before | After |
|-----|--------|-------|
| MLAT sync header | `0x10 0x03` (DLE ETX) | `0x10 0x02` (DLE STX) |
| Message type byte | Raw DF type (`(b>>3) & 0x1F`) | `0x31` short (≤7B) / `0x32` long (14B) |
| Inbound parser sync | Only `0x10 0x03` | `0x10 0x02` or `0x10 0x03` |

Tests: all 96 pass.

---

## Constant Changes

File: `src/tracking/validity.rs`

| Constant | Current | New (match readsb C) |
|----------|---------|---------------------|
| `TRACK_STALE` | `15_000` (15s) | `60_000` (60s) |
| `TRACK_EXPIRE` | `60_000` (60s) | `300_000` (300s) |

---

## Cycle 1 — Fix `a.seen` Not Updating

**Root cause:** `update_from_message()` (main path for every CRC-OK message) never updates
`a.seen`. It's only updated in `update_squawk_only()`. Most aircraft never change squawk,
so `seen` stays at creation timestamp → expire after 60s regardless of continued messages.

### RED — Tests

**New test** (`tests/tracking_compat/test_tracker.rs`):
`test_tracker_seen_updates_on_message` — create aircraft via `update_from_message` at t=1000, assert `a.seen == 1000`

**Existing test update** (`tests/tracking_compat/test_tracker.rs:195`):
Current test: `let removed = tracker.remove_stale(61000); assert_eq!(removed, 2);`
With new TRACK_EXPIRE=300s, calling `remove_stale(61000)` should expect **0 removed**
(aircraft created at t=1000, 61000 < 1000 + 300000).

Update the test in two separate changes:
1. Change assertion to expect 0 removed at `remove_stale(61000)` → **RED** (gets 2 with old 60s expire)
2. After constants change → **GREEN** (gets 0 with new 300s expire)
3. Then add a separate assertion at `remove_stale(301001)` expecting 2 removed → **RED** (gets 0 at current 60s expire)
4. After constants change → **GREEN** (gets 2 at new 300s expire boundary)

In practice, steps 1+2 and 3+4 can be done as one RED→GREEN cycle each.

### GREEN — Implementation

File: `src/tracking/validity.rs:3-4`
```rust
pub const TRACK_STALE: i64 = 60_000;   // 60s (was 15_000)
pub const TRACK_EXPIRE: i64 = 300_000; // 300s (was 60_000)
```

File: `src/tracking/tracker.rs:157` — after `a.messages += 1;`
```rust
a.seen = now;
```

### REFACTOR

None needed. One-line additions. Both `new()` and the new `with_position()` (added in Cycle 5)
will automatically use the new constants.

---

## Cycle 2 — Signal Level Pipeline

**Root cause:** Signal level is never computed. `demodulate2400` returns raw bytes with no signal.
`ModesMessage::signal_level` is always `0.0`. `Aircraft::add_signal()` exists but is never called.
Stats accumulator ignores signal = 0.0 readings, so always shows `sig: 0.0 dBFS`.

### RED — Tests

**`tests/demod_compat.rs` — `test_demod_signal_level`:**

Build a magnitude buffer with a realistic preamble + message pattern.
The preamble at 2 MHz (0.5 µs/sample) has pulses at samples 0, 2, 7, 9:

```
preamble: [5000, 50, 5000, 50, 50, 50, 50, 5000, 50, 5000]
           ^^^^     ^^^^               ^^^^     ^^^^
            0        2                    7        9
```

Followed by 112 Manchester-encoded bit samples for a DF17 long message,
then pad the rest. The preamble peaks should average ~5000.

Then call `demodulate2400` and assert:
- One message returned: `(msg_bytes, signal)`
- `signal` ≈ 5000 (average of 4 preamble peaks)
- `msg_bytes` is 14 bytes (long frame)

**`tests/modes_compat.rs` — `test_parse_modes_message_with_signal`:**

This test will fail to **compile** until `parse_modes_message` accepts a `signal_level`
parameter. That's the RED state — a compilation failure is legitimate TDD red.

```rust
let msg_bytes = [/* valid DF17 bytes */];
let signal = 5000.0;
let result = parse_modes_message(&msg_bytes, 112, &crc_engine, signal);
assert_eq!(result.unwrap().message.signal_level, 5000.0);
```

**`tests/tracking_compat/test_tracker.rs` — `test_tracker_signal_level`:**
Create message with `signal_level = 5000.0`, update tracker, assert `a.get_signal_db() > 0.0`.

### GREEN — Implementation

File: `src/demod/demod_2400.rs`:
1. `decode_message()` return `Option<(Vec<u8>, f64)>` — signal = average of `mag[0]`, `mag[2]`, `mag[7]`, `mag[9]`
2. `demodulate2400()` return `Vec<(Vec<u8>, f64)>`
3. Both call sites inside `demodulate2400` updated (112-bit and 56-bit paths)

File: `src/modes/parser.rs`:
- Accept `signal_level: f64` parameter
- Set `mm.signal_level = signal_level` after initializing but before decoding

File: `src/main.rs`:
- Destructure `for (raw_msg, signal)` from demod messages
- Pass `signal` to `parse_modes_message()`

File: `src/tracking/tracker.rs:157` — in `update_from_message`:
```rust
if msg.signal_level > 0.0 {
    a.add_signal(msg.signal_level, now);
}
```

### REFACTOR

- `test_demod_no_messages_in_noise`, `test_demod_empty_buffer`, `test_demod_small_buffer`:
  `.is_empty()` still works on `Vec<(Vec<u8>, f64)>` — no change needed.
- The `parse_modes_message` signature change affects any other call site (search for
  `parse_modes_message` to confirm no other callers exist — there are two, both in main.rs).

---

## Cycle 3 — `cpr_ok` Stat Always 0%

**Root cause:** `ModesMessage::cpr_decoded` defaults to `false` and is never set to `true`.
The parser sets `cpr_valid = true` in position decoders but not `cpr_decoded`.
Stats accumulator counts `cpr_decoded` as "CPR OK", so always 0%.

### RED — Tests

`tests/modes_compat.rs`:
```rust
test_cpr_decoded_set_on_airborne_position
test_cpr_decoded_set_on_surface_position
```
Parse a DF17 position message (airborne and surface), assert `msg.cpr_decoded == true`.

Use the existing `parse_modes_message` function with the signal_level parameter added
in Cycle 2 (or 0.0 if not yet added — the tests for Cycle 3 should be written AFTER
Cycle 2 green is verified).

### GREEN — Implementation

File: `src/modes/parser.rs:109` — in `decode_airborne_position`:
```rust
mm.cpr_decoded = true;
```

File: `src/modes/parser.rs:99` — in `decode_surface_position`:
```rust
mm.cpr_decoded = true;
```

### REFACTOR

None needed. One-line additions.

---

## Cycle 4 — DF Percentages >100%

**Root cause:** `StatsAccumulator::messages_by_type: [u32; 32]` is never windowed —
counts accumulate from process start. `aggregate()` prunes `message_timestamps` to 60s
for `msgs_per_sec`, but the DF counts remain cumulative. The `format_low` formula
`(count / (msgs_per_sec * 60)) * 100` divides cumulative counts by a 60s window → >100%.

### RED — Test

`tests/console/stats_accumulator.rs` — `test_stats_accumulator_df_windowed`:

```rust
let mut acc = StatsAccumulator::new();
let start = Instant::now();

// 100 DF17 at t=0
for _ in 0..100 {
    acc.record_message(17, true, false, false, false, 0xA43EA2, -12.1, start);
}

// 2 DF11 at t=90s (outside the 60s window from t=60 to t=120)
let t90 = start + Duration::from_secs(90);
acc.record_message(11, true, false, false, false, 0xA6C311, -12.1, t90);
acc.record_message(11, true, false, false, false, 0xA6C312, -12.1, t90);

// Aggregate at t=120s — window = 60s spanning t=60 to t=120
let stats = acc.aggregate(start + Duration::from_secs(120), 0, start);

// DF17 count should be ~0 (all windowed out)
assert!(stats.df_distribution.iter().all(|(df, _)| *df != 17));
// DF11 should have ~2 entries
assert!(stats.df_distribution.iter().any(|(df, count)| *df == 11 && *count == 2));
```

### GREEN — Implementation

File: `src/console/stats_accumulator.rs`:

Remove `messages_by_type: [u32; 32]`, add:
```rust
df_types: VecDeque<(Instant, u8)>,
```

In `record_message`: replace counter increment with:
```rust
self.df_types.push_back((now, df));
```

In `aggregate`: prune `df_types` alongside `message_timestamps` with same 60s window:
```rust
while self.df_types.front().map(|&(t, _)| t < window_start).unwrap_or(false) {
    self.df_types.pop_front();
}
let mut df_dist: [u32; 32] = [0; 32];
for &(_, df) in &self.df_types {
    if (df as usize) < 32 {
        df_dist[df as usize] += 1;
    }
}
let df_distribution: Vec<(u8, u32)> = df_dist.iter().enumerate()
    .filter(|(_, &c)| c > 0)
    .map(|(i, &c)| (i as u8, c))
    .collect();
```

In `reset()` — add:
```rust
self.df_types.clear();
```

### REFACTOR

- The `df_types` `VecDeque` needs the same `new()` initialization as `message_timestamps`
- The `reset()` method must clear `df_types` — confirmed above
- Verify existing `test_stats_accumulator_basic` still passes: it records 3 messages at
  t=0 and t=1, aggregates at t=60 — all within window, so both DF17=2 and DF11=1 appear

---

## Cycle 5 — Wire Receiver Position + Relative CPR + Range Check

**Root cause:** `READSB_LAT` / `READSB_LON` are parsed from env but never used.
`Tracker::user_lat`/`user_lon` always stay at `0.0`. `decode_cpr_relative()` exists
at `cpr/decode.rs:253` but is never called. No range filtering, no distance computation.

In the original readsb C, receiver position enables:
1. **CPR relative decode** — instant position fix from a single frame (no even+odd wait)
2. **Range filtering** — suppress aircraft beyond `max_range`
3. **Distance/bearing** — in JSON output

Impact on "zero planes": without relative decode, global decode requires both even AND odd
frame. If marginal reception captures only one frame type per aircraft pass, position is
never resolved → aircraft have no lat/lon → many downstream displays suppress them.

### RED — Tests

`tests/tracking_compat/test_tracker.rs`:

1. `test_tracker_cpr_relative_decode`
   - Create Tracker with `user_lat=48.0, user_lon=10.0`
   - Send a single even CPR frame with valid raw values
   - Assert position IS resolved immediately (`a.lat != 0.0`, `a.lon != 0.0`)
   - This fails (RED) because `decode_cpr_relative` is never called in `update_position`

2. `test_tracker_range_filter`
   - Create Tracker with `user_lat=48.0, user_lon=10.0`
   - Set `tracker.max_range = 1.0` (1 meter — impossibly small for any real aircraft)
   - Send a single even CPR frame
   - Assert position is NOT stored (`a.lat == 0.0`, `a.lon == 0.0`)
   - This fails (RED) because range check isn't implemented yet

3. Existing `test_tracker_cpr_pairing` must remain unchanged:
   - Uses `Tracker::new()` → `user_lat=0.0, user_lon=0.0` → relative decode guard skips
   - Even frame alone → no position → green
   - Even+odd pair → global decode → position resolved → green

### GREEN — Implementation

File: `src/tracking/mod.rs` — add haversine distance:
```rust
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos()
        * lat2.to_radians().cos()
        * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}
```

File: `src/tracking/tracker.rs`:

Add `with_position` constructor:
```rust
pub fn with_position(lat: f64, lon: f64) -> Self {
    Tracker {
        user_lat: lat,
        user_lon: lon,
        ..Tracker::new()
    }
}
```

Rewrite `update_position` to add relative decode + range check before global decode:
```rust
fn update_position(&self, a: &mut Aircraft, msg: &ModesMessage, now: i64) {
    if !msg.cpr_valid { return; }

    if msg.cpr_odd {
        a.cpr_odd_lat = msg.cpr_lat;
        a.cpr_odd_lon = msg.cpr_lon;
    } else {
        a.cpr_even_lat = msg.cpr_lat;
        a.cpr_even_lon = msg.cpr_lon;
    }

    if self.user_lat != 0.0 || self.user_lon != 0.0 {
        if let Some((lat, lon)) = decode_cpr_relative(
            self.user_lat, self.user_lon,
            msg.cpr_lat as i32, msg.cpr_lon as i32,
            if msg.cpr_odd { 1 } else { 0 },
            msg.cpr_type == CprType::Surface,
        ) {
            let passes_range = self.max_range <= 0.0
                || haversine_distance(self.user_lat, self.user_lon, lat, lon) <= self.max_range;
            if passes_range {
                a.lat = lat; a.lon = lon;
                a.position_valid.update(msg.source, now);
                a.seen_pos = now;
            }
        }
    }

    if (a.cpr_even_lat != 0 || a.cpr_even_lon != 0)
        && (a.cpr_odd_lat != 0 || a.cpr_odd_lon != 0)
    {
        if let Some((lat, lon)) = decode_cpr_airborne(
            a.cpr_even_lat as i32, a.cpr_even_lon as i32,
            a.cpr_odd_lat as i32, a.cpr_odd_lon as i32, 0,
        ) {
            if lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                a.lat = lat; a.lon = lon;
                a.position_valid.update(msg.source, now);
                a.seen_pos = now;
            }
        }
    }
}
```

Update `use` imports in tracker.rs:
```rust
use crate::cpr::{decode_cpr_airborne, decode_cpr_relative};
use super::{Aircraft, AircraftRegistry, TRACK_EXPIRE};
```

Note: `Tracker::new()` sets `user_lat: 0.0, user_lon: 0.0` → the relative decode guard
`self.user_lat != 0.0 || self.user_lon != 0.0` evaluates to `false` → existing
`test_tracker_cpr_pairing` continues to work unchanged (must wait for even+odd pair).

File: `src/main.rs:76`:
```rust
let tracker = Arc::new(Tracker::new());
```
→
```rust
let tracker = Arc::new(Tracker::with_position(
    config.lat.unwrap_or(0.0),
    config.lon.unwrap_or(0.0),
));
```

### REFACTOR

- `update_position` now has two decode paths. Verify readability: the relative decode
  (instant) is first, global decode (more accurate) overrides. Both write to `a.lat/lon`.
  If relative decode writes a value and global decode later overwrites it with the same
  value, there's no harm — but consider whether the global decode should be conditional
  on having BOTH frames (it already is, via the guard on even+odd non-zero).
- Consider extracting the range-check + position-write into a helper method like
  `set_aircraft_position(a, lat, lon, source, now)` to avoid the triple-repetition
  of `a.lat = lat; a.lon = lon; a.position_valid.update(...); a.seen_pos = now;`.

---

## Complete File Change Map

| File | Changes | Cycle |
|------|---------|-------|
| `src/tracking/validity.rs` | `TRACK_STALE 15_000→60_000`, `TRACK_EXPIRE 60_000→300_000` | 1 |
| `src/tracking/tracker.rs` | `a.seen = now`, `add_signal()` call, `with_position()` ctor, rewrite `update_position` with relative decode + range check | 1, 2, 5 |
| `src/tracking/mod.rs` | Add `pub fn haversine_distance()` | 5 |
| `src/demod/demod_2400.rs` | Return `Vec<(Vec<u8>, f64)>`, compute signal from 4 preamble peaks | 2 |
| `src/modes/parser.rs` | Accept `signal_level` param, set `cpr_decoded=true` in position decoders | 2, 3 |
| `src/main.rs` | Use `Tracker::with_position()`, destructure signal from messages | 2, 5 |
| `src/console/stats_accumulator.rs` | Replace `[u32; 32]` with `VecDeque<(Instant, u8)>`, window-prune in `aggregate()`, add `df_types.clear()` in `reset()` | 4 |
| `tests/tracking_compat/test_tracker.rs` | Update stale timing, add seen/position/signal tests | 1, 2, 5 |
| `tests/demod_compat.rs` | Add `test_demod_signal_level` | 2 |
| `tests/modes_compat.rs` | Add signal + `cpr_decoded` tests | 2, 3 |

## Tests NOT modified (should pass as-is)

| Test | Why unaffected |
|------|---------------|
| `test_demod_no_messages_in_noise` | `.is_empty()` works on `Vec<(_, _)>` |
| `test_demod_empty_buffer` | same |
| `test_demod_small_buffer` | same |
| `test_registry_remove_stale` | Uses custom expire param 30000, not `TRACK_EXPIRE` |
| `test_tracker_cpr_pairing` | Uses `Tracker::new()` — `user_lat=0.0` → relative decode skipped |
| All stats accumulator tests | Don't inspect df_distribution directly |
| `test_tracker_creates_aircraft` | No position/signal/seened assertions |
| All per-field updater tests | No position/signal/seened assertions |
| `test_tracker_update_squawk_only` | Separate path, not refactored |
