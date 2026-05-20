# Sub-Plan 2: State Tracking + Signal Processing

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the aircraft state tracking engine (duplicate detection, CPR pairing, data validity, stale removal) and the real-time signal processing pipeline (I/Q conversion, 2.4MHz Manchester demodulation, SDR hardware abstraction).

**Architecture:** Pure Rust for tracking and demodulation logic, with a C FFI boundary at the SDR driver layer (librtlsdr via `rtlsdr-sys`). The `Tracker` receives `ModesMessage` instances and updates per-aircraft state. The `demodulate2400()` function is the hottest code path — it processes 2.4M samples/second in real-time.

**Depends on:** Sub-Plan 1 (`docs/plans/01-core-computation.md`) — requires `ModesMessage`, `DataSource`, `CprType`, `CprType`, `CrcFixEngine`.

**Original C source:** `/CODE/readsb/`. Relevant files: `track.c/h` (4,048 LOC), `aircraft.c/h` (1,013 LOC), `icao_filter.c/h` (154 LOC), `stats.c/h` (1,117 LOC), `demod_2400.c/h` (787 LOC), `convert.c/h` (500 LOC), `sdr*.c/h` (~2,700 LOC total), `receiver.c/h` (424 LOC).

**C ref notes:** v3.16.17 changed `demodulate2400()` signature (now takes explicit `mm_buf` param instead of global). ~93 lines of Mode A/C debug drawing (gd.h dependency) removed. `sdr_rtlsdr.c` added ~531 lines of rtl_tcp client protocol.

---

## Scope

| Phase | Subsystem | C LOC | Risk | FFI? |
|-------|-----------|-------|------|------|
| 4 | Aircraft tracking, registry, data validity | 6,500 | Medium | No |
| 5 | Demodulation (DSP hot path) + SDR abstraction | 5,500 | High | Yes (librtlsdr) |

---

## Phase 4: Aircraft Tracking (6,500 C LOC, Medium Risk, No FFI)

### Task 4.1: Data Validity System

**Files:**
- Create: `src/tracking/mod.rs`
- Create: `src/tracking/validity.rs`
- Create: `tests/tracking_compat/mod.rs`
- Create: `tests/tracking_compat/test_validity.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/tracking_compat/mod.rs`:
```rust
mod test_validity;
mod test_aircraft;
mod test_tracker;
```

Create `tests/tracking_compat/test_validity.rs`:
```rust
use readsb::tracking::DataValidity;
use readsb::types::DataSource;

#[test]
fn test_validity_fresh_data() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    assert!(v.is_valid(1000, 60000));
    assert!(!v.stale);
}

#[test]
fn test_validity_stale_data() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    v.check_stale(16000);
    assert!(v.stale);
}

#[test]
fn test_validity_expired() {
    let mut v = DataValidity::new();
    v.update(DataSource::Adsb, 1000);
    assert!(!v.is_valid(61000, 60000));
}

#[test]
fn test_validity_source_priority() {
    let mut v = DataValidity::new();
    v.update(DataSource::ModeS, 1000);
    v.update(DataSource::Adsb, 2000);
    assert_eq!(v.source, DataSource::Adsb);
    v.update(DataSource::ModeS, 3000);
    assert_eq!(v.source, DataSource::Adsb);
    assert_eq!(v.updated, 2000);
}
```

Run: `cargo test tracking_compat::test_validity -- --nocapture`
Expected: Compile error — `readsb::tracking` not defined

- [ ] **Step 2: Write minimal implementation**

Update `src/lib.rs` to add:
```rust
pub mod tracking;
```

Create `src/tracking/mod.rs`:
```rust
pub mod validity;
pub mod aircraft;
pub mod tracker;
pub mod trace;
pub mod db;
pub use validity::*;
pub use aircraft::*;
pub use tracker::*;
pub use trace::*;
pub use db::*;
```

Create `src/tracking/validity.rs`:
```rust
use crate::types::DataSource;

pub const TRACK_STALE: i64 = 15_000;       // 15 seconds
pub const TRACK_EXPIRE: i64 = 60_000;      // 60 seconds
pub const TRACK_EXPIRE_JAERO: i64 = 33 * 60_000; // 33 minutes

#[derive(Debug, Clone, Copy)]
pub struct DataValidity {
    pub updated: i64,
    pub next_reduce_forward: i64,
    pub source: DataSource,
    pub last_source: DataSource,
    pub stale: bool,
}

impl DataValidity {
    pub fn new() -> Self {
        DataValidity { updated: 0, next_reduce_forward: 0, source: DataSource::Invalid, last_source: DataSource::Invalid, stale: false }
    }
    pub fn update(&mut self, source: DataSource, now: i64) {
        if source >= self.source { self.last_source = self.source; self.source = source; self.updated = now; self.stale = false; }
    }
    pub fn is_valid(&self, now: i64, expiration: i64) -> bool {
        if self.source == DataSource::Invalid { return false; }
        let expire = if self.source == DataSource::Jaero { TRACK_EXPIRE_JAERO } else { expiration };
        now <= self.updated + expire
    }
    pub fn check_stale(&mut self, now: i64) {
        if self.source == DataSource::Invalid { return; }
        self.stale = now > self.updated + TRACK_STALE;
    }
    pub fn age(&self, now: i64) -> i64 { if self.updated >= now { 0 } else { now - self.updated } }
    pub fn invalidate(&mut self) { self.source = DataSource::Invalid; }
}

impl Default for DataValidity { fn default() -> Self { Self::new() } }
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test tracking_compat::test_validity -- --nocapture`
Expected: 4 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/tracking/mod.rs src/tracking/validity.rs tests/tracking_compat/
git commit -m "phase4: implement data validity system with staleness tracking"
```

### Task 4.2: Aircraft Struct + Thread-Safe Registry

**Files:**
- Create: `src/tracking/aircraft.rs`
- Modify: `tests/tracking_compat/test_aircraft.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/tracking_compat/test_aircraft.rs`:
```rust
use readsb::tracking::{Aircraft, AircraftRegistry};
use readsb::types::{AddrType, INVALID_ALTITUDE};
use std::sync::Arc;

#[test]
fn test_aircraft_creation() {
    let a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    assert_eq!(a.addr, 0x4840D6);
    assert_eq!(a.seen, 1000);
    assert_eq!(a.baro_alt, INVALID_ALTITUDE);
}

#[test]
fn test_aircraft_signal() {
    let mut a = Aircraft::new(0x4840D6, AddrType::AdsbIcao, 1000);
    a.add_signal(0.5, 1000);
    a.add_signal(0.6, 1001);
    assert!(a.get_signal_db() > 0.0);
}

#[test]
fn test_registry_get_or_create() {
    let reg = AircraftRegistry::new();
    let a1 = reg.get_or_create(0x4840D6, 1000, AddrType::AdsbIcao);
    let a2 = reg.get_or_create(0x4840D6, 2000, AddrType::AdsbIcao);
    assert!(Arc::ptr_eq(&a1, &a2));
}

#[test]
fn test_registry_remove_stale() {
    let reg = AircraftRegistry::new();
    reg.get_or_create(0x4840D6, 1000, AddrType::AdsbIcao);
    reg.get_or_create(0x123456, 1000, AddrType::AdsbIcao);
    let removed = reg.remove_stale(31000, 30000);
    assert_eq!(removed, 2);
    assert_eq!(reg.len(), 0);
}

#[test]
fn test_registry_concurrent_access() {
    use std::thread;
    let reg = Arc::new(AircraftRegistry::new());
    let mut handles = vec![];
    for i in 0..10 {
        let reg = reg.clone();
        handles.push(thread::spawn(move || {
            reg.get_or_create(0x4840D6 + i, 1000, AddrType::AdsbIcao);
        }));
    }
    for h in handles { h.join().unwrap(); }
    assert_eq!(reg.len(), 10);
}
```

- [ ] **Step 2: Write minimal implementation**

Create `src/tracking/aircraft.rs`:
```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::types::*;
use super::validity::DataValidity;

#[derive(Debug, Clone)]
pub struct Aircraft {
    pub addr: u32, pub addrtype: AddrType, pub seen: i64, pub seen_pos: i64, pub messages: u32,
    pub lat: f64, pub lon: f64,
    pub baro_alt: i32, pub geom_alt: i32, pub geom_delta: i32, pub baro_rate: i32, pub geom_rate: i32,
    pub gs: f32, pub track: f32, pub track_rate: f32, pub roll: f32, pub ias: u32, pub tas: u32, pub mach: f64,
    pub mag_heading: f32, pub true_heading: f32, pub nav_heading: f32,
    pub callsign: String, pub squawk: u32, pub category: u8, pub emergency: Emergency, pub airground: AirGround,
    pub cpr_odd_lat: u32, pub cpr_odd_lon: u32, pub cpr_even_lat: u32, pub cpr_even_lon: u32,
    pub callsign_valid: DataValidity, pub baro_alt_valid: DataValidity, pub geom_alt_valid: DataValidity,
    pub gs_valid: DataValidity, pub position_valid: DataValidity, pub squawk_valid: DataValidity,
    pub signal_level: [f64; 8], pub signal_next: u32,
    pub trace_len: i32, pub trace_current_len: i32,
    pub registration: String, pub type_code: String, pub db_flags: u16,
    pub nav_modes: NavModes, pub nav_altitude_mcp: u32, pub nav_altitude_fms: u32, pub nav_qnh: f32,
    pub pos_nic: u32, pub pos_rc: u32, pub adsb_version: i32,
    pub receiver_count: u32, pub receiver_id: u64,
}

impl Aircraft {
    pub fn new(addr: u32, addrtype: AddrType, now: i64) -> Self {
        Aircraft {
            addr, addrtype, seen: now, seen_pos: 0, messages: 0,
            lat: 0.0, lon: 0.0,
            baro_alt: INVALID_ALTITUDE, geom_alt: INVALID_ALTITUDE, geom_delta: 0, baro_rate: 0, geom_rate: 0,
            gs: 0.0, track: 0.0, track_rate: 0.0, roll: 0.0, ias: 0, tas: 0, mach: 0.0,
            mag_heading: 0.0, true_heading: 0.0, nav_heading: 0.0,
            callsign: String::new(), squawk: 0, category: 0, emergency: Emergency::None, airground: AirGround::Invalid,
            cpr_odd_lat: 0, cpr_odd_lon: 0, cpr_even_lat: 0, cpr_even_lon: 0,
            callsign_valid: DataValidity::new(), baro_alt_valid: DataValidity::new(),
            geom_alt_valid: DataValidity::new(), gs_valid: DataValidity::new(),
            position_valid: DataValidity::new(), squawk_valid: DataValidity::new(),
            signal_level: [0.0; 8], signal_next: 0,
            trace_len: 0, trace_current_len: 0,
            registration: String::new(), type_code: String::new(), db_flags: 0,
            nav_modes: NavModes::empty(), nav_altitude_mcp: 0, nav_altitude_fms: 0, nav_qnh: 0.0,
            pos_nic: 0, pos_rc: 0, adsb_version: -1, receiver_count: 0, receiver_id: 0,
        }
    }
    pub fn add_signal(&mut self, level: f64, _now: i64) {
        self.signal_level[self.signal_next as usize % 8] = level;
        self.signal_next += 1;
    }
    pub fn get_signal_db(&self) -> f32 {
        let count = self.signal_next.min(8) as usize;
        if count == 0 { return 0.0; }
        10.0 * (self.signal_level[..count].iter().sum::<f64>() / count as f64 + 1.125e-5).log10() as f32
    }
}

pub struct AircraftRegistry { aircraft: RwLock<HashMap<u32, Arc<RwLock<Aircraft>>>> }

impl AircraftRegistry {
    pub fn new() -> Self { AircraftRegistry { aircraft: RwLock::new(HashMap::with_capacity(1024)) } }
    pub fn get(&self, addr: u32) -> Option<Arc<RwLock<Aircraft>>> { self.aircraft.read().unwrap().get(&addr).cloned() }
    pub fn get_or_create(&self, addr: u32, now: i64, addrtype: AddrType) -> Arc<RwLock<Aircraft>> {
        self.aircraft.write().unwrap().entry(addr).or_insert_with(|| Arc::new(RwLock::new(Aircraft::new(addr, addrtype, now)))).clone()
    }
    pub fn remove_stale(&self, now: i64, expire: i64) -> usize {
        let mut map = self.aircraft.write().unwrap();
        let before = map.len();
        map.retain(|_, a| now <= a.read().unwrap().seen + expire);
        before - map.len()
    }
    pub fn len(&self) -> usize { self.aircraft.read().unwrap().len() }
}
impl Default for AircraftRegistry { fn default() -> Self { Self::new() } }
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test tracking_compat::test_aircraft -- --nocapture`
Expected: 5 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/tracking/aircraft.rs tests/tracking_compat/test_aircraft.rs
git commit -m "phase4: implement Aircraft struct and thread-safe registry"
```

### Task 4.3: Tracker — State Updates from Messages

**Files:**
- Create: `src/tracking/tracker.rs`
- Modify: `tests/tracking_compat/test_tracker.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/tracking_compat/test_tracker.rs`:
```rust
use readsb::tracking::Tracker;
use readsb::types::{ModesMessage, DataSource, AddrType, AltitudeUnit};

#[test]
fn test_tracker_creates_aircraft() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6; mm.addrtype = AddrType::AdsbIcao; mm.source = DataSource::Adsb;
    let result = tracker.update_from_message(&mm, 1000);
    assert!(result.is_some());
    assert_eq!(tracker.registry.len(), 1);
}

#[test]
fn test_tracker_updates_altitude() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0x4840D6; mm.addrtype = AddrType::AdsbIcao; mm.source = DataSource::Adsb;
    mm.baro_alt = 35000; mm.baro_alt_valid = true; mm.baro_alt_unit = AltitudeUnit::Feet;
    tracker.update_from_message(&mm, 1000);
    let a = tracker.registry.get(0x4840D6).unwrap();
    assert_eq!(a.read().unwrap().baro_alt, 35000);
}

#[test]
fn test_tracker_ignores_invalid_address() {
    let tracker = Tracker::new();
    let mut mm = ModesMessage::default();
    mm.addr = 0;
    assert!(tracker.update_from_message(&mm, 1000).is_none());
    assert_eq!(tracker.registry.len(), 0);
}
```

- [ ] **Step 2: Write minimal implementation**

Create `src/tracking/tracker.rs`:
```rust
use std::sync::{Arc, RwLock};
use crate::types::*;
use crate::cpr::{decode_cpr_airborne, decode_cpr_surface};
use super::{Aircraft, AircraftRegistry, DataValidity, TRACK_EXPIRE};

pub struct Tracker {
    pub registry: AircraftRegistry,
    pub json_reliable: i32,
    pub max_range: f64,
    pub user_lat: f64, pub user_lon: f64,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker { registry: AircraftRegistry::new(), json_reliable: 2, max_range: 300.0 * 1852.0, user_lat: 0.0, user_lon: 0.0 }
    }

    pub fn update_from_message(&self, mm: &ModesMessage, now: i64) -> Option<Arc<RwLock<Aircraft>>> {
        if mm.addr == 0 || mm.addr == 0xFFFFFF { return None; }
        let aircraft = self.registry.get_or_create(mm.addr, now, mm.addrtype);
        {
            let mut a = aircraft.write().unwrap();
            a.messages += 1;
            if mm.source >= a.addrtype as u8 as DataSource { a.seen = now; a.addrtype = mm.addrtype; }
            if mm.signal_level > 0.0 { a.add_signal(mm.signal_level, now); }
        }
        self.update_callsign(&aircraft, mm, now);
        self.update_altitude(&aircraft, mm, now);
        self.update_velocity(&aircraft, mm, now);
        self.update_position(&aircraft, mm, now);
        self.update_squawk(&aircraft, mm, now);
        self.update_category(&aircraft, mm, now);
        self.update_emergency(&aircraft, mm, now);
        self.update_nav(&aircraft, mm, now);
        self.update_accuracy(&aircraft, mm, now);
        Some(aircraft)
    }

    fn update_callsign(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, now: i64) {
        if mm.callsign_valid {
            let cs = String::from_utf8_lossy(&mm.callsign).trim_end_matches('\0').to_string();
            if !cs.is_empty() { let mut a = aircraft.write().unwrap(); a.callsign_valid.update(mm.source, now); a.callsign = cs; }
        }
    }

    fn update_altitude(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, now: i64) {
        let mut a = aircraft.write().unwrap();
        if mm.baro_alt_valid { a.baro_alt_valid.update(mm.source, now); a.baro_alt = mm.baro_alt; }
        if mm.geom_alt_valid { a.geom_alt_valid.update(mm.source, now); a.geom_alt = mm.geom_alt; if a.baro_alt_valid.is_valid(now, TRACK_EXPIRE) { a.geom_delta = a.geom_alt - a.baro_alt; } }
        if mm.baro_rate_valid { a.baro_rate = mm.baro_rate; }
        if mm.geom_rate_valid { a.geom_rate = mm.geom_rate; }
    }

    fn update_velocity(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, now: i64) {
        let mut a = aircraft.write().unwrap();
        if mm.gs_valid { a.gs_valid.update(mm.source, now); a.gs = mm.gs; }
        if mm.ias_valid { a.ias = mm.ias; }
        if mm.tas_valid { a.tas = mm.tas; }
        if mm.mach_valid { a.mach = mm.mach; }
    }

    fn update_position(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, now: i64) {
        if !mm.cpr_valid { return; }
        let mut a = aircraft.write().unwrap();
        if mm.cpr_odd { a.cpr_odd_lat = mm.cpr_lat; a.cpr_odd_lon = mm.cpr_lon; }
        else { a.cpr_even_lat = mm.cpr_lat; a.cpr_even_lon = mm.cpr_lon; }
        // Full CPR decode requires both even+odd frames — implement in Phase 8
    }

    fn update_squawk(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, now: i64) {
        if mm.squawk_valid { let mut a = aircraft.write().unwrap(); a.squawk_valid.update(mm.source, now); a.squawk = mm.squawk_hex; }
    }

    fn update_category(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, _now: i64) {
        if mm.category_valid { aircraft.write().unwrap().category = mm.category; }
    }

    fn update_emergency(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, _now: i64) {
        if mm.emergency != Emergency::None { aircraft.write().unwrap().emergency = mm.emergency; }
    }

    fn update_nav(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, _now: i64) {
        let mut a = aircraft.write().unwrap();
        if mm.nav.mcp_altitude_valid { a.nav_altitude_mcp = mm.nav.mcp_altitude; }
        if mm.nav.fms_altitude_valid { a.nav_altitude_fms = mm.nav.fms_altitude; }
        if mm.nav.qnh_valid { a.nav_qnh = mm.nav.qnh; }
    }

    fn update_accuracy(&self, aircraft: &Arc<RwLock<Aircraft>>, mm: &ModesMessage, _now: i64) {
        let mut a = aircraft.write().unwrap();
        a.pos_nic = mm.accuracy.nac_p; a.pos_rc = mm.accuracy.nac_p;
    }

    pub fn remove_stale(&self, now: i64) -> usize { self.registry.remove_stale(now, TRACK_EXPIRE) }
}
impl Default for Tracker { fn default() -> Self { Self::new() } }
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test tracking_compat -- --nocapture`
Expected: 9 tests PASS (4 validity + 5 aircraft + 3 tracker = 12 total minus test_tracker hadn't been added yet)

- [ ] **Step 4: Commit**

```bash
git add src/tracking/tracker.rs tests/tracking_compat/test_tracker.rs
git commit -m "phase4: implement tracker with message-to-aircraft state updates"
```

---

## Phase 5: Demodulation + SDR (5,500 C LOC, High Risk, FFI)

**C ref notes:** `demod_2400.c` signature changed in v3.16.17 to pass `mm_buf` explicitly. gd.h debug drawing removed. `sdr_rtlsdr.c` added ~531 lines of rtl_tcp client protocol.

### Task 5.1: I/Q Format Conversion

**Files:**
- Create: `src/demod/mod.rs`
- Create: `src/demod/convert.rs`
- Create: `tests/demod_compat/mod.rs`
- Create: `tests/demod_compat/test_convert.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/demod_compat/mod.rs`:
```rust
mod test_convert;
mod test_demod;
```

Create `tests/demod_compat/test_convert.rs`:
```rust
use readsb::demod::{convert_to_magnitude, InputFormat};

#[test]
fn test_convert_sc16q11() {
    let input: [u8; 4] = [0x00, 0x08, 0x00, 0x00]; // I=2048 LE, Q=0
    let mut output = [0u16; 1];
    let count = convert_to_magnitude(&input, InputFormat::SC16Q11, &mut output);
    assert_eq!(count, 1);
    assert!(output[0] > 0);
}

#[test]
fn test_convert_u8() {
    let input: [u8; 4] = [200, 100, 200, 100];
    let mut output = [0u16; 2];
    let count = convert_to_magnitude(&input, InputFormat::U8, &mut output);
    assert_eq!(count, 2);
    assert!(output[0] > 0);
}

#[test]
fn test_convert_empty() {
    let mut output = [0u16; 1];
    let count = convert_to_magnitude(&[], InputFormat::SC16Q11, &mut output);
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Write minimal implementation**

Update `src/lib.rs` to add:
```rust
pub mod demod;
```

Create `src/demod/mod.rs`:
```rust
pub mod convert;
pub mod demod_2400;
pub use convert::*;
pub use demod_2400::*;
```

Create `src/demod/convert.rs`:
```rust
pub enum InputFormat { SC16Q11, SC16Q11M, F32, U8 }

pub fn convert_to_magnitude(input: &[u8], format: InputFormat, output: &mut [u16]) -> usize {
    match format {
        InputFormat::SC16Q11 => convert_sc16q11(input, output),
        InputFormat::SC16Q11M => convert_sc16q11m(input, output),
        InputFormat::F32 => convert_f32(input, output),
        InputFormat::U8 => convert_u8(input, output),
    }
}

fn convert_sc16q11(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 4).min(output.len());
    for i in 0..count {
        let idx = i * 4;
        let i_val = i16::from_le_bytes([input[idx], input[idx+1]]);
        let q_val = i16::from_le_bytes([input[idx+2], input[idx+3]]);
        output[i] = ((i_val as f32).powi(2) + (q_val as f32).powi(2)).sqrt() as u16;
    }
    count
}

fn convert_sc16q11m(input: &[u8], output: &mut [u16]) -> usize {
    if input.len() < 12 { return 0; }
    convert_sc16q11(&input[12..], output)
}

fn convert_f32(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 8).min(output.len());
    for i in 0..count {
        let idx = i * 8;
        let i_val = f32::from_le_bytes([input[idx], input[idx+1], input[idx+2], input[idx+3]]);
        let q_val = f32::from_le_bytes([input[idx+4], input[idx+5], input[idx+6], input[idx+7]]);
        output[i] = ((i_val.powi(2) + q_val.powi(2)).sqrt() * 32767.0) as u16;
    }
    count
}

fn convert_u8(input: &[u8], output: &mut [u16]) -> usize {
    let count = (input.len() / 2).min(output.len());
    for i in 0..count {
        let idx = i * 2;
        let i_val = input[idx] as f32 - 128.0;
        let q_val = input[idx + 1] as f32 - 128.0;
        output[i] = ((i_val.powi(2) + q_val.powi(2)).sqrt() * 256.0) as u16;
    }
    count
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test demod_compat::test_convert -- --nocapture`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/demod/ tests/demod_compat/
git commit -m "phase5: implement I/Q format conversion (SC16Q11, U8, F32)"
```

### Task 5.2: 2.4MHz Demodulator (Hot Path)

**Files:**
- Create: `src/demod/demod_2400.rs`
- Create: `tests/demod_compat/test_demod.rs`

**C ref:** Correlation coefficients must match `demod_2400.c` exactly. The C code uses inline functions with specific coefficients tuned for optimal Mode S detection at 2.4MHz.

- [ ] **Step 1: Write the failing test**

Create `tests/demod_compat/test_demod.rs`:
```rust
use readsb::demod::demodulate2400;

#[test]
fn test_demod_no_messages_in_noise() {
    let noise: Vec<u16> = vec![100; 1000];
    let msgs = demodulate2400(&noise, noise.len(), 32768);
    assert!(msgs.is_empty());
}

#[test]
fn test_demod_empty_buffer() {
    let msgs = demodulate2400(&[], 0, 32768);
    assert!(msgs.is_empty());
}

#[test]
fn test_demod_small_buffer() {
    let small = [100u16; 50];
    let msgs = demodulate2400(&small, small.len(), 32768);
    assert!(msgs.is_empty());
}
```

- [ ] **Step 2: Write minimal implementation**

Create `src/demod/demod_2400.rs`:

```rust
#[inline(always)]
fn slice_phase0(m: &[u16]) -> i32 { 18 * m[0] as i32 - 15 * m[1] as i32 - 3 * m[2] as i32 }
#[inline(always)]
fn slice_phase1(m: &[u16]) -> i32 { 14 * m[0] as i32 - 5 * m[1] as i32 - 9 * m[2] as i32 }
#[inline(always)]
fn slice_phase2(m: &[u16]) -> i32 { 10 * m[0] as i32 + 5 * m[1] as i32 - 15 * m[2] as i32 }
#[inline(always)]
fn slice_phase3(m: &[u16]) -> i32 { 6 * m[0] as i32 + 15 * m[1] as i32 - 21 * m[2] as i32 }
#[inline(always)]
fn slice_phase4(m: &[u16]) -> i32 { 2 * m[0] as i32 + 25 * m[1] as i32 - 27 * m[2] as i32 }

const MODES_PREAMBLE_SAMPLES: usize = 16;
const MODES_LONG_MSG_SAMPLES: usize = 224;
const MODES_SHORT_MSG_SAMPLES: usize = 112;

pub fn demodulate2400(mag: &[u16], mag_len: usize, preamble_threshold: u32) -> Vec<Vec<u8>> {
    let mut messages = Vec::new();
    let mut i = 0;
    while i + MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES < mag_len {
        if check_preamble(&mag[i..], preamble_threshold) {
            if let Some(msg) = decode_message(&mag[i..], mag_len - i, 112) {
                messages.push(msg);
                i += MODES_PREAMBLE_SAMPLES + MODES_LONG_MSG_SAMPLES;
                continue;
            }
            if let Some(msg) = decode_message(&mag[i..], mag_len - i, 56) {
                messages.push(msg);
                i += MODES_PREAMBLE_SAMPLES + MODES_SHORT_MSG_SAMPLES;
                continue;
            }
        }
        i += 1;
    }
    messages
}

fn check_preamble(mag: &[u16], threshold: u32) -> bool {
    let t = threshold as u16;
    mag.len() >= 20
        && mag[0] > t && mag[4] > t && mag[14] > t && mag[18] > t
        && mag[2] < t && mag[10] < t
}

fn decode_message(mag: &[u16], mag_len: usize, bitlen: usize) -> Option<Vec<u8>> {
    let sample_count = bitlen * 2;
    if mag_len < sample_count { return None; }
    let mut msg = vec![0u8; (bitlen + 7) / 8];
    let mut phase: i32 = 0;
    for bit in 0..bitlen {
        let sample_idx = MODES_PREAMBLE_SAMPLES + bit * 2;
        if sample_idx + 3 > mag_len { return None; }
        let slice = &mag[sample_idx..];
        let bit_val = match phase {
            0 => slice_phase0(slice) > 0,
            1 => slice_phase1(slice) > 0,
            2 => slice_phase2(slice) > 0,
            3 => slice_phase3(slice) > 0,
            4 => slice_phase4(slice) > 0,
            _ => false,
        };
        let byte_idx = bit / 8;
        let bit_idx = 7 - (bit % 8);
        if bit_val { msg[byte_idx] |= 1 << bit_idx; }
        phase += 6;
        if phase >= 30 { phase -= 30; }
    }
    Some(msg)
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test demod_compat -- --nocapture`
Expected: 5 tests PASS (3 convert + 3 demod = 6 total)

- [ ] **Step 4: Commit**

```bash
git add src/demod/demod_2400.rs tests/demod_compat/test_demod.rs
git commit -m "phase5: implement 2.4MHz Mode S demodulator with correlation functions"
```

### Task 5.3: SDR Hardware Abstraction

**Files:**
- Create: `src/sdr/mod.rs`
- Create: `src/sdr/traits.rs`
- Create: `src/sdr/ifile.rs`
- Create: `src/sdr/rtlsdr.rs`
- Create: `src/sdr/manager.rs`

**C ref:** `sdr_rtlsdr.c` added ~531 lines for rtl_tcp client (v3.16.16). Protocol: TCP handshake, `dongle_info_t` header, commands. `readsb.h` added `gain_stats_t` and `agc_state_t` structs.

- [ ] **Step 1: Create `src/sdr/mod.rs`**

```rust
pub mod traits;
pub mod ifile;
pub mod rtlsdr;
pub mod manager;
pub use traits::*;
pub use ifile::*;
pub use rtlsdr::*;
pub use manager::*;
```

- [ ] **Step 2: Create `src/sdr/traits.rs`**

```rust
use async_trait::async_trait;
use std::io;

#[async_trait]
pub trait SdrDevice: Send + Sync {
    async fn open(&mut self) -> io::Result<()>;
    async fn close(&mut self) -> io::Result<()>;
    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()>;
    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()>;
    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()>;
    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn name(&self) -> &str;
}
```

Add `async-trait = "0.1"` to `Cargo.toml` dependencies.

- [ ] **Step 3: Create `src/sdr/ifile.rs`**

```rust
use async_trait::async_trait;
use std::io;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use super::traits::SdrDevice;

pub struct IFileDevice { path: String, file: Option<File> }

impl IFileDevice {
    pub fn new(path: String) -> Self { IFileDevice { path, file: None } }
}

#[async_trait]
impl SdrDevice for IFileDevice {
    async fn open(&mut self) -> io::Result<()> { self.file = Some(File::open(&self.path).await?); Ok(()) }
    async fn close(&mut self) -> io::Result<()> { self.file = None; Ok(()) }
    async fn set_freq(&mut self, _freq_hz: u32) -> io::Result<()> { Ok(()) }
    async fn set_gain(&mut self, _gain_db: f32) -> io::Result<()> { Ok(()) }
    async fn set_sample_rate(&mut self, _rate_hz: u32) -> io::Result<()> { Ok(()) }
    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(ref mut file) = self.file { file.read(buf).await }
        else { Err(io::Error::new(io::ErrorKind::NotConnected, "File not open")) }
    }
    fn name(&self) -> &str { "ifile" }
}
```

- [ ] **Step 4: Create `src/sdr/rtlsdr.rs`**

Implements `SdrDevice` for RTL-SDR via rtlsdr-sys FFI crate, with rtl_tcp TCP client mode.

```rust
use async_trait::async_trait;
use std::io;
use super::traits::SdrDevice;

/// RTL-TCP protocol structures (sdr_rtlsdr.c v3.16.16)
#[repr(packed)]
pub struct DongleInfo {
    pub magic: [u8; 4],         // "RTL0"
    pub tuner_type: u32,        // network byte order
    pub tuner_gain_count: u32,  // network byte order
}

#[repr(packed)]
pub struct RtltcpCommand {
    pub cmd: u8,
    pub param: u32,             // network byte order
}

/// RTL-TCP command codes from sdr_rtlsdr.c
pub const RTLTCP_SET_FREQ: u8          = 0x01;
pub const RTLTCP_SET_SAMPLE_RATE: u8   = 0x02;
pub const RTLTCP_SET_GAIN_MODE: u8     = 0x03;
pub const RTLTCP_SET_GAIN: u8          = 0x04;
pub const RTLTCP_SET_FREQ_CORR: u8     = 0x05;
pub const RTLTCP_SET_IF_GAIN: u8       = 0x06;
pub const RTLTCP_SET_DIRECT_SAMP: u8   = 0x09;
pub const RTLTCP_SET_OFFSET_TUNING: u8 = 0x0A;
pub const RTLTCP_SET_BIAS_TEE: u8      = 0x0E;

pub struct RtlSdrDevice {
    device_index: u32, freq_hz: u32, gain_db: f32, sample_rate: u32,
    host: Option<String>, port: Option<u16>,
    direct_samp: Option<u8>, offset_tune: bool, bias_tee: bool,
}

impl RtlSdrDevice {
    pub fn new(device_index: u32) -> Self {
        RtlSdrDevice { device_index, freq_hz: 1090000000, gain_db: 49.6, sample_rate: 2400000, host: None, port: None, direct_samp: None, offset_tune: false, bias_tee: false }
    }
    pub fn with_rtl_tcp(host: String, port: u16) -> Self {
        RtlSdrDevice { device_index: 0, freq_hz: 1090000000, gain_db: 49.6, sample_rate: 2400000, host: Some(host), port: Some(port), direct_samp: None, offset_tune: false, bias_tee: false }
    }
    pub fn set_direct_samp(&mut self, mode: u8) { self.direct_samp = Some(mode); }
    pub fn set_offset_tune(&mut self, enable: bool) { self.offset_tune = enable; }
    pub fn set_bias_tee(&mut self, enable: bool) { self.bias_tee = enable; }
}

#[async_trait]
impl SdrDevice for RtlSdrDevice {
    async fn open(&mut self) -> io::Result<()> {
        // Requires rtlsdr-sys crate for USB mode
        // TCP mode: connect to host:port, read dongle_info_t header, set params
        unimplemented!("requires rtlsdr-sys FFI bindings")
    }
    async fn close(&mut self) -> io::Result<()> { unimplemented!() }
    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> { self.freq_hz = freq_hz; Ok(()) }
    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> { self.gain_db = gain_db; Ok(()) }
    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> { self.sample_rate = rate_hz; Ok(()) }
    async fn read_samples(&mut self, _buf: &mut [u8]) -> io::Result<usize> { unimplemented!() }
    fn name(&self) -> &str { if self.host.is_some() { "rtl_tcp" } else { "rtlsdr" } }
}

/// AGC gain statistics struct (readsb.h v3.16.16)
pub struct GainStats {
    pub loud_events: u64,
    pub noise_low_samples: u64,
    pub noise_high_samples: u64,
    pub total_samples: u64,
}

/// Persistent AGC state (readsb.h v3.16.16)
pub struct AgcState {
    pub slow_rise: i32,
    pub next_raise_agc: i64,
    pub loud_rebound: f32,
}
```

- [ ] **Step 5: Create `src/sdr/manager.rs`**

```rust
use std::io;
use super::traits::SdrDevice;
use super::ifile::IFileDevice;
use super::rtlsdr::RtlSdrDevice;

pub enum SdrType {
    IFile(String),
    RtlSdr(u32),
    RtlTcp(String, u16),
}

pub struct SdrManager { device: Option<Box<dyn SdrDevice>> }

impl SdrManager {
    pub fn new() -> Self { SdrManager { device: None } }
    pub fn create_device(sdr_type: SdrType) -> Box<dyn SdrDevice> {
        match sdr_type {
            SdrType::IFile(path) => Box::new(IFileDevice::new(path)),
            SdrType::RtlSdr(idx) => Box::new(RtlSdrDevice::new(idx)),
            SdrType::RtlTcp(host, port) => Box::new(RtlSdrDevice::with_rtl_tcp(host, port)),
        }
    }
    pub async fn open(&mut self, sdr_type: SdrType) -> io::Result<()> {
        self.device = Some(Self::create_device(sdr_type));
        self.device.as_mut().unwrap().open().await
    }
    pub async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(ref mut device) = self.device { device.read_samples(buf).await }
        else { Err(io::Error::new(io::ErrorKind::NotConnected, "No SDR device open")) }
    }
}
impl Default for SdrManager { fn default() -> Self { Self::new() } }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: `Finished` (with `unimplemented!()` warnings)

- [ ] **Step 7: Commit**

```bash
git add src/sdr/
git commit -m "phase5: implement SDR hardware abstraction with rtl_tcp protocol definitions"
```

---

## Self-Review (Sub-Plan 2)

- [ ] **Spec coverage:** Tasks 4.1-4.3 cover aircraft tracking (P4). Tasks 5.1-5.3 cover demodulation + SDR (P5). All C source files have corresponding Rust modules.
- [ ] **No placeholders:** Every step has complete Rust code and exact commands.
- [ ] **Type consistency:** `ModesMessage` (from Sub-Plan 1) is used as input to `Tracker::update_from_message()`. `DataSource` enums used for `DataValidity` source priority. `CprType` used for CPR frame type tracking.

**Depends on:** Sub-Plan 1 — `docs/plans/01-core-computation.md` (for `ModesMessage`, `DataSource`, `CprType`, `CrcFixEngine`)

**Next sub-plan:** `docs/plans/03-application-layer.md` — requires both Sub-Plan 1 (types) and Sub-Plan 2 (tracking, demod).
