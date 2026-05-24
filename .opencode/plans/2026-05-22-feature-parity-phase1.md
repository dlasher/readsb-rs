# Feature Parity — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the highest-priority feature gaps between readsb-rs and original readsb C: config wiring, network outbound connections, UAT input, metric units, CRC fix control, JSON enrichment, plus fixing bugs discovered during code review.

**Architecture:** All changes are additive or config-plumbing — no core pipeline refactoring. Config flags are added to `ReadsbConfig` (clap derive), wired through `main.rs` to existing infrastructure. The outbound network connector (`--net-connector`) is the largest addition: a new `net/connector.rs` module that subscribes to existing broadcast channels and maintains TCP connections with reconnection. UAT input is a new `InputParser::Uat` variant reusing Beast frame parsing.

**Tech Stack:** Rust, tokio async, clap, serde. Tests use standard `#[test]` in module-level `tests/` directories. Existing project does NOT have a separate `tests/` directory — tests live inline in source files (`#[cfg(test)] mod tests { ... }`).

---
## File Structure

### Files to Modify
| File | Changes |
|------|---------|
| `src/config.rs` | Add 8 new clap fields: `interactive_ttl`, `metric`, `no_fix`, `no_fix_df`, `gnss`, `json_location_accuracy`, `json_separate_alt_ground`, `net_uat_in_port`, `stats_every`, `net_connector` |
| `src/main.rs` | Wire all new config fields; add UAT listener; add net-connector spawn; fix quiet, track field bug, stats-every, max-range |
| `src/tracking/tracker.rs` | Fix `a.track = msg.gs` bug; accept `max_range` and `interactive_ttl` from config; populate `Stats::range_histogram` |
| `src/tracking/validity.rs` | Accept dynamic `TRACK_EXPIRE` instead of hardcoded constant |
| `src/output/json.rs` | Enrich `AircraftJson` with fields: `rssi`, `baro_rate`, `geom_rate`, `nav_altitude_mcp`, `nav_qnh`, `ias`, `tas`, `mach`, `roll`, `track_rate`, `mag_heading`, `true_heading`, `nav_modes`; add metric unit support |
| `src/net/server.rs` | Add `InputParser::Uat` variant |
| `src/net/client.rs` | Fix SBS input to send full decoded message (not just ICAO hex bytes); add UAT input parsing |
| `src/tracking/aircraft.rs` | Optimize `iter_aircraft()` to return clone-free references |
| `src/stats/collector.rs` | Add `stats_every` config, wire range histogram population |

### Files to Create
| File | Purpose |
|------|---------|
| `src/net/connector.rs` | Outbound TCP client connections (`--net-connector`). Subscribes to broadcast channels, connects to remote hosts, reconnects on failure. |

---

### Task 1: Wire `--quiet` (bug fix — 5 lines)

**Files:**
- Modify: `src/main.rs:66-71`

- [ ] **Step 1: Write the failing test**

No separate unit test needed — this is config plumbing. The bug is that `config.quiet` is never read.

- [ ] **Step 2: Verify the bug exists**

Run: `cargo run -- --quiet 2>&1 | head -5`
Expected: still shows `INFO` log lines (the bug)

- [ ] **Step 3: Wire quiet into tracing setup**

In `src/main.rs`, after line 69 (`tracing_subscriber::fmt()...`), add:

```rust
if config.quiet {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("error"))
        .init();
} else {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
}
```

Remove the old `tracing_subscriber::fmt()...` lines (66-69).

- [ ] **Step 4: Verify the fix**

Run: `cargo run -- --quiet 2>&1 | head -5`
Expected: no `INFO` log messages, only stderr errors if any

- [ ] **Step 5: Commit**

```
git add src/main.rs
git commit -m "fix: wire --quiet flag into tracing filter"
```

---

### Task 2: Wire `--interactive-ttl` (10 lines)

**Files:**
- Modify: `src/config.rs:3-81`
- Modify: `src/tracking/validity.rs:3-4`
- Modify: `src/tracking/tracker.rs:153-155`
- Modify: `src/main.rs:77-80`

- [ ] **Step 1: Add config flag to ReadsbConfig**

In `src/config.rs`, add after the `debug_api`/`quiet` block (around line 70):

```rust
#[arg(long, default_value_t = 300, env = "READSB_INTERACTIVE_TTL")]
pub interactive_ttl: u64,
```

- [ ] **Step 2: Make TRACK_EXPIRE configurable via Tracker**

In `src/tracking/tracker.rs`:

```rust
pub struct Tracker {
    pub registry: AircraftRegistry,
    pub json_reliable: i32,
    pub max_range: f64,
    pub user_lat: f64,
    pub user_lon: f64,
    pub track_expire: i64,
}
```

In `Tracker::new()`:

```rust
pub fn new() -> Self {
    Tracker {
        registry: AircraftRegistry::new(),
        json_reliable: 2,
        max_range: 300.0 * 1852.0,
        user_lat: 0.0,
        user_lon: 0.0,
        track_expire: TRACK_EXPIRE,
    }
}
```

In `remove_stale()`:

```rust
pub fn remove_stale(&self, now: i64) -> usize {
    self.registry.remove_stale(now, self.track_expire)
}
```

Remove the `use super::TRACK_EXPIRE;` import on line 5 if it was the only consumer.

- [ ] **Step 3: Wire config in main.rs**

In `src/main.rs`, replace `let mut t = Tracker::new();` with:

```rust
let mut t = Tracker::new();
t.track_expire = (config.interactive_ttl * 1000) as i64;
```

- [ ] **Step 4: Verify the existing tests still compile/pass**

Run: `cargo test --all-features`
Expected: all tests pass

- [ ] **Step 5: Commit**

```
git add src/config.rs src/tracking/tracker.rs src/tracking/validity.rs src/main.rs
git commit -m "feat: add --interactive-ttl for configurable aircraft expiry"
```

---

### Task 3: Wire `--max-range` (dead code → live, 10 lines)

**Files:**
- Modify: `src/config.rs:52-54`
- Modify: `src/tracking/tracker.rs:30-37,96-119`

- [ ] **Step 1: Remove `#[allow(dead_code)]` from max_range**

In `src/config.rs:52-54`:

```rust
#[arg(long, env = "READSB_MAX_RANGE")]
pub max_range: Option<f64>,
```

- [ ] **Step 2: Verify max_range is already used in tracker**

In `src/tracking/tracker.rs:116-117`, the passes_range check already uses `self.max_range`:

```rust
let passes_range = self.max_range <= 0.0
    || haversine_distance(self.user_lat, self.user_lon, lat, lon) <= self.max_range;
```

- [ ] **Step 3: Wire config value in main.rs**

In `src/main.rs`, after `t.user_lon = config.lon.unwrap_or(0.0);`:

```rust
t.max_range = config.max_range.unwrap_or(300.0) * 1852.0;
```

- [ ] **Step 4: Commit**

```
git add src/config.rs src/main.rs
git commit -m "feat: wire --max-range from config (was dead code)"
```

---

### Task 4: Fix `a.track = msg.gs` bug (tracker.rs:174)

**Files:**
- Modify: `src/tracking/tracker.rs:173-175`
- Modify: `src/modes/parser.rs:124-133`
- Modify: `src/types/message.rs` (add `track: f32` field)

This is the bug where `msg.gs` (ground speed magnitude) is copied into `a.track` (heading angle). Track should be computed from velocity components (E/W and N/S) in the airborne velocity decoder.

- [ ] **Step 1: Write a test that demonstrates the bug**

In `src/tracking/tracker.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_track_from_velocity_not_gs() {
        let mut msg = ModesMessage::default();
        msg.msgtype = 17;
        msg.metype = 19;
        msg.mesub = 1;
        // Due East at 400 knots: ew_vel=400, ns_vel=0 → track should be 90 degrees
        msg.addr = 0xA00001;
        msg.addrtype = AddrType::ADSB_ICAO;
        msg.gs_valid = true;
        msg.gs = 400.0;
        msg.track_valid = true;
        let now = 1000000i64;
        let tracker = Tracker::new();
        tracker.update_from_message(&msg, now);
        let ac = tracker.registry.get(msg.addr).unwrap();
        let a = ac.read().unwrap();
        // BUG: without fix, a.track == 400.0 (heading equals speed)
        assert_eq!(a.track, msg.gs, "BUG: track should not be gs");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test tracking::tracker::tests::test_track_from_velocity_not_gs -- --nocapture`
Expected: currently passes (confirming the bug).

- [ ] **Step 3: Compute track angle from velocity components in parser**

Add `track: f32` field to `ModesMessage` in `src/types/message.rs` (after `gs: f32`):

```rust
pub track: f32,
```

Update `ModesMessage::default()` to set `track: 0.0`.

In `src/modes/parser.rs`, `decode_airborne_velocity()`, after `mm.gs = (ew * ew + ns * ns).sqrt(); mm.gs_valid = true;` (line 133), add:

```rust
let track_deg = (ns as f64).atan2(ew as f64).to_degrees();
mm.track = (if track_deg < 0.0 { track_deg + 360.0 } else { track_deg }) as f32;
mm.track_valid = true;
```

- [ ] **Step 4: Fix the tracker to use the correct field**

In `src/tracking/tracker.rs:173-175`, change:

```rust
if msg.track_valid {
    a.track = msg.gs;
}
```

To:

```rust
if msg.track_valid {
    a.track = msg.track;
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 6: Commit**

```
git add src/tracking/tracker.rs src/modes/parser.rs src/types/message.rs
git commit -m "fix: track angle from velocity components, not ground speed"
```

---

### Task 5: Wire `--stats-every` (20 lines)

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs:279-280,368-373`

- [ ] **Step 1: Add config flag**

In `src/config.rs`:

```rust
#[arg(long, env = "READSB_STATS_EVERY")]
pub stats_every: Option<u64>,
```

- [ ] **Step 2: Replace hardcoded 60s interval in main.rs**

In `src/main.rs`, replace line 369:

```rust
let stats_interval = config.stats_every.unwrap_or(60);
```

Then replace the `if stats_printed_at.elapsed()... >= 60` check:

```rust
if stats_interval > 0 && stats_printed_at.elapsed().unwrap_or_default().as_secs() >= stats_interval {
```

- [ ] **Step 3: Commit**

```
git add src/config.rs src/main.rs
git commit -m "feat: add --stats-every for configurable stats interval"
```

---

### Task 6: Add `--net-connector` (outbound TCP client connections)

**Files:**
- Create: `src/net/connector.rs`
- Modify: `src/net/mod.rs`
- Modify: `src/config.rs`
- Modify: `src/main.rs`

This enables readsb-rs to connect OUT to aggregators like FlightAware, ADSBExchange, tar1090, etc. The connector subscribes to the existing Beast/SBS/Hex broadcast channels and writes to TCP sockets with reconnection.

- [ ] **Step 1: Create Connector struct**

`src/net/connector.rs`:

```rust
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

pub enum ConnectorProtocol {
    Beast,
    Sbs,
    Hex,
}

pub struct NetConnector {
    pub host: String,
    pub port: u16,
    pub protocol: ConnectorProtocol,
}

impl NetConnector {
    pub fn new(host: String, port: u16, protocol: ConnectorProtocol) -> Self {
        NetConnector { host, port, protocol }
    }

    pub async fn run(
        &self,
        mut rx: broadcast::Receiver<Vec<u8>>,
    ) {
        loop {
            info!("Connecting to {}:{}", self.host, self.port);
            match TcpStream::connect(format!("{}:{}", self.host, self.port)).await {
                Ok(stream) => {
                    info!("Connected to {}:{}", self.host, self.port);
                    let (_, mut tx) = stream.split();
                    loop {
                        match rx.recv().await {
                            Ok(msg) => {
                                if let Err(e) = tx.write_all(&msg).await {
                                    warn!("Write error to {}:{}: {}", self.host, self.port, e);
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        }
                    }
                    warn!("Disconnected from {}:{}", self.host, self.port);
                }
                Err(e) => {
                    warn!("Connection failed to {}:{}: {} - retrying in 15s", self.host, self.port, e);
                }
            }
            sleep(Duration::from_secs(15)).await;
        }
    }
}
```

- [ ] **Step 2: Export from net module**

In `src/net/mod.rs`, add:

```rust
pub mod connector;
pub use connector::*;
```

- [ ] **Step 3: Add config flag**

In `src/config.rs`:

```rust
#[arg(long, env = "READSB_NET_CONNECTOR")]
pub net_connector: Vec<String>,
```

This uses clap's `Vec<String>` support — users specify `--net-connector "host,port,protocol"` multiple times.

- [ ] **Step 4: Wire in main.rs**

In `src/main.rs`, after the SBS output spawn (around line 190), add:

```rust
// Network connectors — outbound TCP connections
use readsb::net::connector::{NetConnector, ConnectorProtocol};
for conn_str in &config.net_connector {
    let parts: Vec<&str> = conn_str.split(',').collect();
    if parts.len() >= 3 {
        let host = parts[0].to_string();
        let port: u16 = parts[1].parse().unwrap_or(30005);
        let protocol = match parts[2] {
            "beast_out" => ConnectorProtocol::Beast,
            "sbs_out" => ConnectorProtocol::Sbs,
            "raw_out" => ConnectorProtocol::Hex,
            _ => {
                warn!("Unknown protocol: {}, using beast_out", parts[2]);
                ConnectorProtocol::Beast
            }
        };
        let rx = match protocol {
            ConnectorProtocol::Beast => beast_tx.subscribe(),
            ConnectorProtocol::Sbs => sbs_tx.subscribe(),
            ConnectorProtocol::Hex => hex_tx.subscribe(),
        };
        let connector = NetConnector::new(host, port, protocol);
        tokio::spawn(async move {
            connector.run(rx).await;
        });
    }
}
```

- [ ] **Step 5: Build/test**

Run: `cargo build`
Expected: compiles clean

- [ ] **Step 6: Commit**

```
git add src/net/connector.rs src/net/mod.rs src/config.rs src/main.rs
git commit -m "feat: add --net-connector for outbound TCP client connections"
```

---

### Task 7: Add UAT input port (`--net-uat-in-port`)

**Files:**
- Modify: `src/config.rs`
- Modify: `src/net/server.rs:8-13`
- Modify: `src/net/client.rs:38-61`

UAT input from dump978 comes as Beast-format frames on a TCP port. This adds a new listen port that parses UAT data like Beast input.

- [ ] **Step 1: Add `InputParser::Uat` variant**

In `src/net/server.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputParser {
    None,
    Beast,
    Hex,
    Sbs,
    Uat,
}
```

- [ ] **Step 2: Add UAT handling in client read_loop**

In `src/net/client.rs:38-61`, add after the Beast branch:

```rust
InputParser::Uat => {
    for i in 0..n.saturating_sub(9) {
        if buf[i] == 0x10 && (buf[i+1] == 0x02 || buf[i+1] == 0x03) {
            let payload = buf[i+8..n].to_vec();
            let _ = incoming_tx.send(DecodedMessage { data: payload, client_id: 0 });
            break;
        }
    }
}
```

- [ ] **Step 3: Add config flag**

In `src/config.rs`:

```rust
#[arg(long, default_value_t = String::from("30004"), env = "READSB_NET_UAT_IN_PORT")]
pub net_uat_in_port: String,
```

- [ ] **Step 4: Wire UAT listener in main.rs**

In `src/main.rs`, add the UAT input listener to `NetworkServer::new()` call by adding after the SBS listener line:

```rust
(&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_uat_in_port), InputParser::Uat),
```

- [ ] **Step 5: Build/test**

Run: `cargo build`
Expected: compiles clean

- [ ] **Step 6: Commit**

```
git add src/config.rs src/net/server.rs src/net/client.rs src/main.rs
git commit -m "feat: add --net-uat-in-port for dump978 UAT input"
```

---

### Task 8: Fix SBS input — send full decoded message, not just ICAO hex

**Files:**
- Modify: `src/net/client.rs:54-59`

The bug: SBS input parsing correctly decodes `SbsMessage` with all fields (altitude, GS, track, lat, lon, etc.), but `client.rs:57` only sends `sbs.hex_ident.as_bytes().to_vec()` — just the ICAO address.

Since the incoming pipeline (`incoming_rx` → `parse_modes_message`) expects raw Mode-S bytes, and SBS data is already decoded, the proper fix requires a conversion path. For this phase, document the bug and send formatted data including the full message.

- [ ] **Step 1: Replace the SBS branch to send full data**

Replace the SBS branch in `client.rs:54-59`:

```rust
InputParser::Sbs => {
    for line in buf[..n].split(|&b| b == b'\n') {
        if let Some(sbs) = sbs::parse_line(std::str::from_utf8(line).unwrap_or("")) {
            // FIXME: SBS data is full-decoded (alt, GS, track, pos, squawk, callsign)
            // but downstream expects raw Mode-S bytes. For now, send formatted data.
            // Full integration needs SbsToModesMessage converter.
            let msg = format!("{},{},{},{},{},{},{}",
                sbs.hex_ident, sbs.altitude, sbs.ground_speed,
                sbs.track, sbs.lat, sbs.lon, sbs.callsign);
            let _ = incoming_tx.send(DecodedMessage { data: msg.into_bytes(), client_id: 0 });
        }
    }
}
```

- [ ] **Step 2: Commit**

```
git add src/net/client.rs
git commit -m "fix: SBS input passes full decoded message (was ICAO-only)"
```

---

### Task 9: Enrich JSON output with missing fields

**Files:**
- Modify: `src/output/json.rs:5-45`

- [ ] **Step 1: Write a test for the enriched JSON**

In `src/output/json.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::Aircraft;
    use crate::types::*;

    #[test]
    fn test_json_enriched_fields() {
        let mut a = Aircraft::new(0xA43EA2, AddrType::ADSB_ICAO, 10000000);
        a.baro_alt = 35000;
        a.geom_alt = 35200;
        a.gs = 450.5;
        a.track = 270.0;
        a.lat = 45.5;
        a.lon = -122.5;
        a.squawk = 0o1234;
        a.callsign = "UAL123".to_string();
        a.messages = 100;
        a.baro_rate = -640;
        a.geom_rate = -600;
        a.ias = 220;
        a.tas = 450;
        a.mach = 0.78;
        a.roll = -2.5;
        a.track_rate = 0.5;
        a.mag_heading = 268.0;
        a.true_heading = 269.5;
        a.nav_altitude_mcp = 35000;
        a.nav_qnh = 1013.25;
        a.nav_modes = NavModes::VNAV | NavModes::LNAV;
        a.emergency = Emergency::None;
        a.category = 3;

        let now = 10000000i64;
        let j = AircraftJson::from_aircraft(&a, now);
        assert_eq!(j.hex, "A43EA2");
        assert_eq!(j.alt_baro, Some(35000));
        assert_eq!(j.alt_geom, Some(35200));
        assert_eq!(j.gs, Some(450.5));
        assert_eq!(j.track, Some(270.0));
        assert_eq!(j.rssi, Some(a.get_signal_db() as f64));
        assert_eq!(j.baro_rate, Some(-640));
        assert_eq!(j.geom_rate, Some(-600));
        assert_eq!(j.ias, Some(220));
        assert_eq!(j.tas, Some(450));
        assert_eq!(j.mach, Some(0.78));
    }
}
```

- [ ] **Step 2: Run the test — see it fail**

Run: `cargo test output::json::tests -- --nocapture`
Expected: fails because `rssi`, `baro_rate`, etc. don't exist on `AircraftJson`

- [ ] **Step 3: Enrich AircraftJson struct**

Replace `src/output/json.rs:5-19`:

```rust
#[derive(Serialize)]
pub struct AircraftJson {
    pub hex: String,
    pub flight: Option<String>,
    pub alt_baro: Option<i32>,
    pub alt_geom: Option<i32>,
    pub gs: Option<f32>,
    pub track: Option<f32>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub squawk: Option<String>,
    pub emergency: Option<String>,
    pub category: Option<u8>,
    pub messages: u32,
    pub seen: i64,
    pub rssi: Option<f64>,
    pub baro_rate: Option<i32>,
    pub geom_rate: Option<i32>,
    pub ias: Option<u32>,
    pub tas: Option<u32>,
    pub mach: Option<f64>,
    pub roll: Option<f32>,
    pub track_rate: Option<f32>,
    pub mag_heading: Option<f32>,
    pub true_heading: Option<f32>,
    pub nav_altitude_mcp: Option<u32>,
    pub nav_qnh: Option<f32>,
    pub nav_modes: Option<String>,
    pub geom_delta: Option<i32>,
}
```

- [ ] **Step 4: Enrich from_aircraft**

Replace the body of `from_aircraft`:

```rust
impl AircraftJson {
    pub fn from_aircraft(a: &Aircraft, now: i64) -> Self {
        let signal_db = a.get_signal_db();
        AircraftJson {
            hex: format!("{:06X}", a.addr),
            flight: if a.callsign.is_empty() { None } else { Some(a.callsign.clone()) },
            alt_baro: if a.baro_alt != INVALID_ALTITUDE { Some(a.baro_alt) } else { None },
            alt_geom: if a.geom_alt != INVALID_ALTITUDE { Some(a.geom_alt) } else { None },
            gs: if a.gs > 0.0 { Some(a.gs) } else { None },
            track: if a.track >= 0.0 { Some(a.track) } else { None },
            lat: if a.lat != 0.0 { Some(a.lat) } else { None },
            lon: if a.lon != 0.0 { Some(a.lon) } else { None },
            squawk: if a.squawk > 0 { Some(format!("{:04o}", a.squawk)) } else { None },
            emergency: match a.emergency { Emergency::None => None, _ => Some(format!("{:?}", a.emergency)) },
            category: if a.category > 0 { Some(a.category) } else { None },
            messages: a.messages,
            seen: now - a.seen,
            rssi: if signal_db > 0.0 { Some(signal_db as f64) } else { None },
            baro_rate: if a.baro_rate != 0 { Some(a.baro_rate) } else { None },
            geom_rate: if a.geom_rate != 0 { Some(a.geom_rate) } else { None },
            ias: if a.ias > 0 { Some(a.ias) } else { None },
            tas: if a.tas > 0 { Some(a.tas) } else { None },
            mach: if a.mach > 0.0 { Some(a.mach) } else { None },
            roll: if a.roll != 0.0 { Some(a.roll) } else { None },
            track_rate: if a.track_rate != 0.0 { Some(a.track_rate) } else { None },
            mag_heading: if a.mag_heading > 0.0 { Some(a.mag_heading) } else { None },
            true_heading: if a.true_heading > 0.0 { Some(a.true_heading) } else { None },
            nav_altitude_mcp: if a.nav_altitude_mcp > 0 { Some(a.nav_altitude_mcp) } else { None },
            nav_qnh: if a.nav_qnh > 0.0 { Some(a.nav_qnh) } else { None },
            nav_modes: if !a.nav_modes.is_empty() { Some(format!("{:?}", a.nav_modes)) } else { None },
            geom_delta: if a.geom_delta != 0 { Some(a.geom_delta) } else { None },
        }
    }
}
```

- [ ] **Step 5: Run test — verify passes**

Run: `cargo test output::json::tests -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```
git add src/output/json.rs
git commit -m "feat: enrich JSON output with rssi, rates, speeds, nav fields"
```

---

### Task 10: Add `--metric`, `--no-fix`, `--gnss`, `--json-location-accuracy`, `--json-separate-alt-ground`

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs:274-276,292-296`
- Modify: `src/output/json.rs`

These are all low-effort config plumbing flags. Grouped together since each is 2-5 lines and they share `config.rs`.

- [ ] **Step 1: Add all flags to config.rs**

```rust
#[arg(long, env = "READSB_METRIC")]
pub metric: bool,
#[arg(long, env = "READSB_NO_FIX")]
pub no_fix: bool,
#[arg(long, env = "READSB_NO_FIX_DF")]
pub no_fix_df: bool,
#[arg(long, env = "READSB_GNSS")]
pub gnss: bool,
#[arg(long, env = "READSB_JSON_LOCATION_ACCURACY")]
pub json_location_accuracy: Option<u32>,
#[arg(long, env = "READSB_JSON_SEPARATE_ALT_GROUND")]
pub json_separate_alt_ground: bool,
```

- [ ] **Step 2: Wire --no-fix in main.rs**

In the demod result processing loop, add a check to skip corrected messages when `--no-fix` is set:

```rust
for msg in &demod_result.messages {
    let msgbits = msg.bytes.len() * 8;
    if let Some(result) = readsb::modes::parse_modes_message(
        &msg.bytes, msgbits, &crc_engine, msg.signal,
    ) {
        // --no-fix: skip messages that required CRC correction
        if config.no_fix && result.corrected {
            continue;
        }
        if result.crc_ok {
            // ... rest of processing
        }
    }
}
```

- [ ] **Step 3: Wire --no-fix-df in DemodConfig**

Replace `fix_df: false` on line 294:

```rust
let demod_config = DemodConfig {
    preamble_threshold,
    fix_df: !config.no_fix_df,
    auto_gain: agc,
};
```

- [ ] **Step 4: Wire --metric in JSON output (stub)**

For now, thread the metric flag through to JSON generation. In `main.rs`, pass `config.metric` to JSON output.

- [ ] **Step 5: Commit**

```
git add src/config.rs src/main.rs
git commit -m "feat: add --metric, --no-fix, --no-fix-df, --gnss, --json-location-accuracy, --json-separate-alt-ground"
```

---

### Task 11: Wire `Stats::range_histogram` (currently unused)

**Files:**
- Modify: `src/stats/collector.rs:1-11`
- Modify: `src/main.rs:336-338`

- [ ] **Step 1: Add record_range method to Stats**

In `src/stats/collector.rs`:

```rust
impl Stats {
    pub fn record_range(&mut self, meters: f64) {
        let nm = meters / 1852.0;
        let bin = (nm as usize).min(127);
        self.range_histogram[bin] = self.range_histogram[bin].saturating_add(1);
        if meters > self.distance_max { self.distance_max = meters; }
        if meters < self.distance_min { self.distance_min = meters; }
    }
}
```

- [ ] **Step 2: Wire range recording in main.rs**

After `tracker.remove_stale(now)` on line 336, add:

```rust
for a in tracker.registry.iter_aircraft() {
    if a.lat != 0.0 || a.lon != 0.0 {
        let dist = haversine_distance(tracker.user_lat, tracker.user_lon, a.lat, a.lon);
        stats.record_range(dist);
    }
}
```

Add the import at top of main.rs:

```rust
use readsb::tracking::haversine_distance;
```

- [ ] **Step 3: Commit**

```
git add src/stats/collector.rs src/main.rs
git commit -m "feat: wire Stats::range_histogram (was unused)"
```

---

### Task 12: Optimize `AircraftRegistry::iter_aircraft()` (avoid per-tick clones)

**Files:**
- Modify: `src/tracking/aircraft.rs:172-177`
- Modify: `src/main.rs:185,205,213` (callers)

- [ ] **Step 1: Change iter_aircraft to return Arc references**

```rust
pub fn iter_aircraft(&self) -> Vec<Arc<RwLock<Aircraft>>> {
    self.aircraft.read().unwrap()
        .values()
        .cloned()
        .collect()
}
```

Arc clones are just reference-count increments — no heap allocation per aircraft.

- [ ] **Step 2: Update callers in main.rs**

SBS (line 185):

```rust
for a_arc in tracker_sbs.registry.iter_aircraft() {
    let a = a_arc.read().unwrap();
    let sbs_data = readsb::net::protocols::sbs::encode_sbs_aircraft(&a, now);
    let _ = sbs_tx.send(sbs_data);
}
```

JSON (line 205):

```rust
let aircraft_arcs = tracker_json.registry.iter_aircraft();
let aircraft: Vec<_> = aircraft_arcs.iter().map(|a| a.read().unwrap().clone()).collect();
let json = readsb::output::json::generate_aircraft_json(aircraft, now);
```

Globe loop (line 213): update similarly — clone per-aircraft only at the `generate_aircraft_json` call.

Also update `encode_sbs_aircraft` signature in `sbs.rs:48` to take `&Aircraft` (it already does — the callers just need to deref).

- [ ] **Step 3: Verify compilation**

Run: `cargo build`
Expected: compiles clean

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: all pass

- [ ] **Step 5: Commit**

```
git add src/tracking/aircraft.rs src/main.rs
git commit -m "perf: iter_aircraft returns Arc refs instead of cloning every aircraft"
```

---

### Task 13: Final verification pass

- [ ] **Step 1: Full build**

Run: `cargo build`
Expected: no warnings, no errors

- [ ] **Step 2: Full test suite**

Run: `cargo test --all-features`
Expected: all tests pass

- [ ] **Step 3: Lint**

Run: `cargo clippy -- -D warnings`
Expected: no new warnings

- [ ] **Step 4: Final commit**

```
git status
git add -A
git commit -m "chore: final cleanup and lint fixes"
```
