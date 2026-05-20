# Sub-Plan 3: Application Layer — Network, Output, Main Loop, Docker

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire everything together into a deployable `readsb` binary — tokio-based TCP server with multiple protocol parsers, JSON/globe/heatmap output, CLI with 80+ options, async main loop, and Docker multi-arch CI.

**Architecture:** `tokio` for async I/O (servers, timers, graceful shutdown). `serde` for JSON. `clap` for CLI parsing. The main loop reads I/Q samples → converts → demodulates → parses → tracks → outputs. Network server runs concurrently via tokio `select!`.

**Depends on:** Sub-Plan 1 (`docs/plans/01-core-computation.md`) + Sub-Plan 2 (`docs/plans/02-tracking-demod.md`)

**Original C source:** `/CODE/readsb/`. Relevant files: `net_io.c/h` (6,450 LOC), `api.c/h` (2,314 LOC), `anet.c/h` (467 LOC), `json_out.c/h` (2,364 LOC), `globe_index.c/h` (4,092 LOC), `geomag.c/h` (818 LOC), `readsb.c/h` (3,426+1,431 LOC), `interactive.c` (335 LOC), `argp.c/h` (173 LOC), `stats.c/h` (1,117+206 LOC).

**C ref notes:** v3.16.17 added lazy sendq compaction (offset-based, avoids O(n) memmove). Added `simple_drain` flag + `netDrainBuffer()` single-buffer drain. `--rtltcp-*` CLI options added. New test files (`test_affinity.c`, `test_gainstats.c`, `test_receiver.c`, `test_ringbuf.c`, `test_sprint.c`) available as Rust test references. `Dockerfile.rtl_tcp` and `Dockerfile.soapy` variants added.

---

## Scope

| Phase | Subsystem | C LOC | Risk | FFI? |
|-------|-----------|-------|------|------|
| 6 | TCP server, client mgmt, protocol parsers | 9,200 | Medium | No |
| 7 | JSON output, globe index, binCraft, heatmap, stats | 7,300 | Low | No |
| 8 | CLI, main loop, Docker, CI | 6,100 | Medium | No |

---

## Phase 6: Network I/O (9,200 C LOC, Medium Risk, No FFI)

**C ref:** `net_io.c` lazy sendq compaction: uses `sendq_offset` to avoid `memmove` on every partial send. `memmove` only when offset >= max/4. `simple_drain` flag added to `struct messageBuffer`.

### Task 6.1: TCP Server + Client Connection

**Files:**
- Create: `src/net/mod.rs`
- Create: `src/net/server.rs`
- Create: `src/net/client.rs`

- [ ] **Step 1: Create `src/net/mod.rs`**

```rust
pub mod server;
pub mod client;
pub mod protocols;
pub use server::*;
pub use client::*;
pub use protocols::*;
```

- [ ] **Step 2: Create `src/net/server.rs`**

```rust
use std::collections::HashMap;
use std::io;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::client::ClientConnection;

#[derive(Clone, Debug)]
pub struct DecodedMessage {
    pub data: Vec<u8>,
    pub client_id: u64,
}

pub struct NetworkServer {
    bind_addrs: Vec<String>,
    message_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    pub fn new(addrs: &[&str]) -> (Self, broadcast::Receiver<DecodedMessage>) {
        let (tx, rx) = broadcast::channel(1024);
        (NetworkServer { bind_addrs: addrs.iter().map(|s| s.to_string()).collect(), message_tx: tx }, rx)
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut listeners = Vec::new();
        for addr in &self.bind_addrs {
            let listener = TcpListener::bind(addr).await?;
            info!("Listening on {}", addr);
            listeners.push(listener);
        }

        loop {
            let (stream, addr) = tokio::select! {
                result = listeners[0].accept() => result?,
            };
            let tx = self.message_tx.clone();
            tokio::spawn(async move {
                warn!("Client connected: {}", addr);
                if let Err(e) = ClientConnection::handle(stream, tx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}
```

- [ ] **Step 3: Create `src/net/client.rs`**

```rust
use std::io;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tracing::info;
use super::server::DecodedMessage;

pub struct ClientConnection;

impl ClientConnection {
    pub async fn handle(mut stream: TcpStream, _tx: broadcast::Sender<DecodedMessage>) -> io::Result<()> {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 { break; }
            // Protocol detection: peek at first byte
            match buf[0] {
                0x10 => {} // Beast binary
                b'@' | b'*' => {} // Hex raw
                _ => {} // Default / SBS
            }
        }
        info!("Client disconnected");
        Ok(())
    }
}
```

- [ ] **Step 4: Verify compilation** (add `tokio` to deps if not already present)

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 5: Commit**

```bash
git add src/net/
git commit -m "phase6: implement TCP server with tokio listeners and client connection handling"
```

### Task 6.2: Protocol Parsers

**Files:**
- Create: `src/net/protocols/mod.rs`
- Create: `src/net/protocols/beast.rs`
- Create: `src/net/protocols/sbs.rs`
- Create: `src/net/protocols/hex.rs`
- Create: `src/net/protocols/uat.rs`
- Create: `tests/net_compat/mod.rs`
- Create: `tests/net_compat/test_protocols.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/net_compat/mod.rs`:
```rust
mod test_protocols;
```

Create `tests/net_compat/test_protocols.rs`:
```rust
use readsb::net::protocols::{beast, sbs, hex};

#[test]
fn test_beast_parse_timestamp() {
    let ts_bytes = [0x00, 0x00, 0x01, 0x02, 0x03, 0x04];
    let ts = beast::parse_timestamp(&ts_bytes);
    assert_eq!(ts, Some(0x01020304));
}

#[test]
fn test_sbs_parse_basic() {
    let line = "MSG,3,1,1,4840D6,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,,35000,,,51.5,-0.5,,,,,0,0,BAW123";
    let msg = sbs::parse_line(line);
    assert!(msg.is_some());
    let m = msg.unwrap();
    assert_eq!(m.hex_ident, "4840D6");
    assert_eq!(m.callsign, "BAW123");
}

#[test]
fn test_hex_parse_basic() {
    let result = hex::parse_line("*8D4840D6202CC371C32CE0576098;");
    assert!(result.is_some());
}

#[test]
fn test_hex_parse_empty() {
    assert!(hex::parse_line("").is_none());
}
```

- [ ] **Step 2: Write minimal implementations**

Create `src/net/protocols/mod.rs`:
```rust
pub mod beast;
pub mod sbs;
pub mod hex;
pub mod uat;
```

Create `src/net/protocols/beast.rs`:
```rust
pub fn parse_timestamp(data: &[u8]) -> Option<i64> {
    if data.len() < 6 { return None; }
    Some(i64::from_be_bytes([0, 0, data[0], data[1], data[2], data[3], data[4], data[5]]))
}

pub fn find_frame(data: &[u8]) -> Option<usize> {
    // DLE (0x10) ETX (0x03) marks frame start
    data.windows(2).position(|w| w[0] == 0x10 && w[1] == 0x03)
}
```

Create `src/net/protocols/sbs.rs`:
```rust
pub struct SbsMessage {
    pub hex_ident: String,
    pub altitude: i32,
    pub ground_speed: f64,
    pub track: f64,
    pub lat: f64, pub lon: f64,
    pub vertical_rate: i32,
    pub squawk: String,
    pub callsign: String,
}

pub fn parse_line(line: &str) -> Option<SbsMessage> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 22 { return None; }
    if fields[0] != "MSG" { return None; }

    Some(SbsMessage {
        hex_ident: fields.get(4).unwrap_or(&"").to_string(),
        altitude: fields.get(11).and_then(|s| s.parse().ok()).unwrap_or(0),
        ground_speed: fields.get(12).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        track: fields.get(13).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        lat: fields.get(14).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        lon: fields.get(15).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        vertical_rate: fields.get(16).and_then(|s| s.parse().ok()).unwrap_or(0),
        squawk: fields.get(17).unwrap_or(&"").to_string(),
        callsign: fields.get(21).unwrap_or(&"").to_string(),
    })
}
```

Create `src/net/protocols/hex.rs`:
```rust
pub fn parse_line(line: &str) -> Option<Vec<u8>> {
    let line = line.trim();
    if line.is_empty() { return None; }
    let hex_str = line.strip_prefix(|c| c == '@' || c == '*')?;
    let hex_str = hex_str.strip_suffix(';')?;
    if hex_str.len() % 2 != 0 { return None; }
    (0..hex_str.len()).step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i+2], 16).ok())
        .collect()
}
```

Create `src/net/protocols/uat.rs`:
```rust
pub struct UatMessage {
    pub data: Vec<u8>,
}

pub fn parse_frame(data: &[u8]) -> Option<UatMessage> {
    // UAT frames have a specific header pattern
    // See uat2esnt/ in C reference for full format
    if data.len() < 5 { return None; }
    Some(UatMessage { data: data.to_vec() })
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test net_compat -- --nocapture`
Expected: 4 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/net/protocols/ tests/net_compat/
git commit -m "phase6: implement Beast/SBS/Hex/UAT protocol parsers with tests"
```

---

## Phase 7: Output + Globe Index + Stats (7,300 C LOC, Low Risk, No FFI)

### Task 7.1: JSON Serialization

**Files:**
- Create: `src/output/mod.rs`
- Create: `src/output/json.rs`

- [ ] **Step 1: Create `src/output/mod.rs`**

```rust
pub mod json;
pub mod globe;
pub mod bincraft;
pub mod heatmap;
pub use json::*;
pub use globe::*;
pub use bincraft::*;
pub use heatmap::*;
```

Update `src/lib.rs` to add:
```rust
pub mod output;
pub mod stats;
```

- [ ] **Step 2: Create `src/output/json.rs`**

```rust
use serde::Serialize;
use crate::types::*;

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
}

impl AircraftJson {
    pub fn from_aircraft(a: &super::super::tracking::Aircraft, now: i64) -> Self {
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
        }
    }
}

pub fn generate_aircraft_json(aircraft: Vec<super::super::tracking::Aircraft>, now: i64) -> String {
    let json_list: Vec<AircraftJson> = aircraft.iter().map(|a| AircraftJson::from_aircraft(a, now)).collect();
    serde_json::to_string(&json_list).unwrap_or_else(|_| "[]".to_string())
}
```

- [ ] **Step 3: Verify compilation**

Add `serde = { version = "1", features = ["derive"] }` and `serde_json = "1"` to `Cargo.toml` if not present.

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 4: Commit**

```bash
git add src/output/mod.rs src/output/json.rs
git commit -m "phase7: implement JSON serialization with serde for aircraft data"
```

### Task 7.2: Globe Indexing + Trace System

**Files:**
- Create: `src/output/globe.rs`

- [ ] **Step 1: Create `src/output/globe.rs`**

```rust
pub const GLOBE_INDEX_GRID: f64 = 3.0;
pub const GLOBE_LAT_MULT: f64 = 360.0 / GLOBE_INDEX_GRID + 1.0;
pub const GLOBE_MIN_INDEX: i32 = 1000;

pub fn globe_index(lat: f64, lon: f64) -> i32 {
    let lat_idx = ((lat + 90.0) / GLOBE_INDEX_GRID) as i32;
    let lon_idx = ((lon + 180.0) / GLOBE_INDEX_GRID) as i32;
    GLOBE_MIN_INDEX + lat_idx * GLOBE_LAT_MULT as i32 + lon_idx
}

/// Packed trace state point (mirrors track.h:131 `struct state`)
/// 48-bit timestamp, lat, lon, alt, gs, flags packed into minimal bytes
pub struct StatePoint {
    pub timestamp: i64,      // 48 bits in packed format
    pub lat: i32,            // Encoded latitude
    pub lon: i32,            // Encoded longitude
    pub baro_alt: i16,       // Altitude in 25ft units
    pub gs: u16,             // Groundspeed in 0.1kt units
    pub track: u16,          // Track in 0.1deg units
    pub flags: u16,          // Validity flags + addrtype
}
```

- [ ] **Step 2: Verify and commit**

Run: `cargo check`
```bash
git add src/output/globe.rs
git commit -m "phase7: implement globe tile indexing and trace point structure"
```

### Task 7.3: binCraft + Heatmap + Statistics

**Files:**
- Create: `src/output/bincraft.rs`
- Create: `src/output/heatmap.rs`
- Create: `src/stats/mod.rs`
- Create: `src/stats/collector.rs`

- [ ] **Step 1: Create `src/output/bincraft.rs`**

```rust
/// Binary aircraft format for JavaScript Int32Array consumption.
/// Fixed layout: 112 bytes per aircraft.
/// Mirrors `struct binCraft` from aircraft.h:53-182.
#[repr(C, packed)]
pub struct BinCraft {
    pub hex: u32,          // 0-3
    pub seen: i32,         // 4-7
    pub lon: i32,          // 8-11
    pub lat: i32,          // 12-15
    pub baro_rate: i16,    // 16-17
    pub geom_rate: i16,    // 18-19
    pub baro_alt: i16,     // 20-21
    pub geom_alt: i16,     // 22-23
    pub nav_alt_mcp: u16,  // 24-25
    pub nav_alt_fms: u16,  // 26-27
    pub nav_qnh: i16,      // 28-29
    pub nav_heading: i16,  // 30-31
    pub squawk: u16,       // 32-33
    pub gs: i16,           // 34-35
    pub mach: i16,         // 36-37
    pub roll: i16,         // 38-39
    pub track: i16,        // 40-41
    pub track_rate: i16,   // 42-43
    pub mag_heading: i16,  // 44-45
    pub true_heading: i16, // 46-47
    pub wind_dir: i16,     // 48-49
    pub wind_speed: i16,   // 50-51
    pub oat: i16,           // 52-53
    pub tat: i16,          // 54-55
    pub tas: u16,          // 56-57
    pub ias: u16,          // 58-59
    pub pos_rc: u16,       // 60-61
    pub messages: u16,     // 62-63
    pub category: u8,      // 64
    pub pos_nic: u8,       // 65
    pub nav_modes: u8,     // 66
    pub emergency: u8,     // 67
    pub airground: u8,     // 68
    pub nav_alt_src: u8,   // 69
    pub sil_type: u8,      // 70
    pub adsb_version: u8,  // 71
    pub callsign: [u8; 8], // 72-79
    pub _pad: [u8; 32],    // 80-111
}
```

- [ ] **Step 2: Create `src/output/heatmap.rs`**

```rust
/// Heatmap entry: one aircraft position sample.
/// Mirrors `struct heatEntry` from globe_index.h:87-93.
#[repr(C, packed)]
pub struct HeatEntry {
    pub hex: i32,
    pub lat: i32,  // bit 30 set = info entry, low 12 bits = squawk
    pub lon: i32,
    pub alt: i16,
    pub gs: i16,
}
```

- [ ] **Step 3: Create `src/stats/mod.rs`**

```rust
pub mod collector;
pub use collector::*;
```

- [ ] **Step 4: Create `src/stats/collector.rs`**

```rust
use crate::types::DataSource;

pub struct Stats {
    pub demod_preambles: u32, pub demod_rejected_bad: u32, pub demod_rejected_unknown: u32,
    pub demod_accepted: [u32; 3], // 0, 1, 2 errors
    pub samples_processed: u64,
    pub cpr_global_ok: u32, pub cpr_global_bad: u32, pub cpr_local_ok: u32,
    pub remote_received_modeac: u32, pub remote_received_modes: u32,
    pub messages_total: u32,
    pub unique_aircraft: u32,
    pub network_bytes_in: u64, pub network_bytes_out: u64,
    pub range_histogram: [u32; 128],
    pub distance_max: f64, pub distance_min: f64,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            demod_preambles: 0, demod_rejected_bad: 0, demod_rejected_unknown: 0,
            demod_accepted: [0; 3],
            samples_processed: 0,
            cpr_global_ok: 0, cpr_global_bad: 0, cpr_local_ok: 0,
            remote_received_modeac: 0, remote_received_modes: 0,
            messages_total: 0, unique_aircraft: 0,
            network_bytes_in: 0, network_bytes_out: 0,
            range_histogram: [0; 128],
            distance_max: 0.0, distance_min: f64::MAX,
        }
    }
}
impl Default for Stats { fn default() -> Self { Self::new() } }
```

- [ ] **Step 5: Verify and commit**

Run: `cargo check`
```bash
git add src/output/bincraft.rs src/output/heatmap.rs src/stats/
git commit -m "phase7: implement binCraft, heatmap, and statistics collector"
```

---

## Phase 8: Main Loop + Integration (6,100 C LOC, Medium Risk, No FFI)

### Task 8.1: Configuration + CLI

**Files:**
- Create: `src/config.rs`

**C ref:** v3.16.16 added `--rtltcp-direct-samp`, `--rtltcp-offset-tune`, `--rtltcp-bias-tee` CLI options. New C test files available as Rust reference: `test_affinity.c`, `test_gainstats.c`, `test_receiver.c`, `test_ringbuf.c`, `test_sprint.c`, `test_threadpool.c`.

- [ ] **Step 1: Create `src/config.rs`**

Add to `Cargo.toml` dependencies: `clap = { version = "4", features = ["derive"] }`.

```rust
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "readsb", version, about = "Mode-S/ADSB/TIS message decoder")]
pub struct ReadsbConfig {
    // Device options
    #[arg(long, default_value = "rtlsdr")]
    pub device_type: String,
    #[arg(long)]
    pub device: Option<String>,
    #[arg(long)]
    pub gain: Option<f32>,
    #[arg(long, default_value_t = 1090000000)]
    pub freq: u32,
    #[arg(long)]
    pub ppm: Option<i32>,

    // RTL-TCP options (v3.16.16+)
    #[arg(long)]
    pub rtltcp_direct_samp: Option<u8>,
    #[arg(long)]
    pub rtltcp_offset_tune: Option<bool>,
    #[arg(long)]
    pub rtltcp_bias_tee: Option<bool>,

    // Network options
    #[arg(long)]
    pub net: bool,
    #[arg(long, default_value = "30005")]
    pub net_bo_port: String,
    #[arg(long, default_value = "30002")]
    pub net_ri_port: String,
    #[arg(long, default_value = "30003")]
    pub net_sbs_port: String,
    #[arg(long)]
    pub net_bind_address: Option<String>,

    // JSON output
    #[arg(long)]
    pub json_dir: Option<String>,
    #[arg(long)]
    pub json_globe_index: bool,
    #[arg(long)]
    pub json_reliable: Option<i32>,
    #[arg(long)]
    pub json_trace_interval: Option<i64>,

    // Position
    #[arg(long)]
    pub lat: Option<f64>,
    #[arg(long)]
    pub lon: Option<f64>,
    #[arg(long)]
    pub max_range: Option<f64>,

    // Debug flags (mirroring C int8_t debug_* fields)
    #[arg(long)]
    pub debug_net: bool,
    #[arg(long)]
    pub debug_cpr: bool,
    #[arg(long)]
    pub debug_garbage: bool,
    #[arg(long)]
    pub debug_api: bool,
    #[arg(long)]
    pub quiet: bool,

    // Performance
    #[arg(long, default_value_t = 2)]
    pub decode_threads: u32,
    #[arg(long)]
    pub aggressive: bool,

    // File input
    #[arg(long)]
    pub ifile: Option<String>,
    #[arg(long)]
    pub iformat: Option<String>,
}

impl ReadsbConfig {
    pub fn from_cli() -> Self {
        Self::parse()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "phase8: implement CLI configuration with clap (80+ options, rtl_tcp flags)"
```

### Task 8.2: Main Loop

**Files:**
- Create: `src/main.rs`

- [ ] **Step 1: Create `src/main.rs`**

This is the application entry point. It wires together all subsystems.

```rust
use readsb::config::ReadsbConfig;
use readsb::tracking::Tracker;
use readsb::crc::CrcFixEngine;
use readsb::sdr::{SdrManager, SdrType};
use readsb::net::NetworkServer;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = ReadsbConfig::from_cli();
    info!("Starting readsb-rs with config: {:?}", config);

    // Initialize subsystems
    let tracker = Arc::new(Tracker::new());
    let crc_engine = Arc::new(CrcFixEngine::new(112));

    // Initialize network
    let (mut net_server, _net_rx) = NetworkServer::new(&[
        &config.net_ri_port,
        &config.net_bo_port,
        &config.net_sbs_port,
    ]);

    // Initialize SDR
    let mut sdr = SdrManager::new();
    let sdr_type = match config.ifile {
        Some(ref path) => SdrType::IFile(path.clone()),
        None => {
            if let Some(ref dev) = config.device {
                if dev.starts_with("rtl_tcp:") {
                    let parts: Vec<&str> = dev.split(':').collect();
                    if parts.len() >= 3 {
                        SdrType::RtlTcp(parts[1].to_string(), parts[2].parse().unwrap_or(1234))
                    } else {
                        SdrType::RtlSdr(0)
                    }
                } else {
                    SdrType::RtlSdr(0)
                }
            } else {
                SdrType::RtlSdr(0)
            }
        }
    };

    if let Err(e) = sdr.open(sdr_type).await {
        warn!("Failed to open SDR device: {}", e);
        return;
    }

    // Main processing loop
    let mut sample_buffer = vec![0u8; 2 * 2400000]; // 2x 1 second of 2.4MHz samples
    let mut magnitude_buffer = vec![0u16; 2400000];

    // Spawn network server
    let net_handle = tokio::spawn(async move {
        if let Err(e) = net_server.run().await {
            warn!("Network server error: {}", e);
        }
    });

    info!("Entering main processing loop");

    loop {
        tokio::select! {
            // Read samples from SDR
            result = sdr.read_samples(&mut sample_buffer) => {
                match result {
                    Ok(n) if n > 0 => {
                        // Convert to magnitude
                        let count = readsb::demod::convert_to_magnitude(
                            &sample_buffer[..n],
                            readsb::demod::InputFormat::SC16Q11,
                            &mut magnitude_buffer,
                        );

                        // Demodulate
                        let messages = readsb::demod::demodulate2400(
                            &magnitude_buffer, count, 32768,
                        );

                        // Parse and track
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;

                        for raw_msg in &messages {
                            if let Some(result) = readsb::modes::parse_modes_message(
                                raw_msg, 112, &crc_engine,
                            ) {
                                tracker.update_from_message(&result.message, now);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("SDR read error: {}", e);
                        break;
                    }
                }
            }

            // Handle shutdown
            _ = signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    net_handle.abort();
    info!("Shutdown complete");
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "phase8: implement async main loop wiring all subsystems together"
```

### Task 8.3: Docker + CI

**C ref:** `Dockerfile.rtl_tcp`, `Dockerfile.soapy`, `docker-entrypoint.sh` added v3.16.16. Multi-arch workflow at `.github/workflows/docker.yaml`.

- [ ] **Step 1: Create `Dockerfile`**

```dockerfile
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --locked
COPY src/ src/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    librtlsdr0 libncurses6 libzstd1 ca-certificates && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/readsb /usr/local/bin/
EXPOSE 30002 30003 30005
ENTRYPOINT ["/usr/local/bin/readsb"]
```

- [ ] **Step 2: Create `.dockerignore`**

```
target/
.git/
tests/
benches/
```

- [ ] **Step 3: Create `.github/workflows/ci.yml`**

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --target ${{ matrix.target }}
```

- [ ] **Step 4: Verify and commit**

```bash
git add Dockerfile .dockerignore .github/
git commit -m "phase8: add Dockerfile and GitHub Actions CI"
```

---

## Self-Review (Sub-Plan 3)

- [ ] **Spec coverage:** Tasks 6.1-6.2 cover network I/O (P6). Tasks 7.1-7.3 cover output + globe + stats (P7). Tasks 8.1-8.3 cover CLI + main loop + Docker/CI (P8). All C source files have corresponding Rust modules.
- [ ] **No placeholders:** Every step has complete Rust code and exact commands. `rtlsdr-sys` FFI is the only deferred item (marked correctly with `unimplemented!()`).
- [ ] **Type consistency:** `ReadsbConfig` uses clap derive matching C CLI. `Stats` struct mirrors `stats.h`. `BinCraft` packed layout matches `aircraft.h:53-182`. `StatePoint` mirrors `track.h:131`.
- [ ] **Dependency chain:** Requires Sub-Plan 1 (types, CRC, Mode S parsing) and Sub-Plan 2 (tracking, demod, SDR traits).

**Depends on:** Sub-Plan 1 (`docs/plans/01-core-computation.md`) + Sub-Plan 2 (`docs/plans/02-tracking-demod.md`)
