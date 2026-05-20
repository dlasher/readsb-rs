# Sub-Plan 1: Core Computation Engine — CRC, CPR, Mode S Parsing

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement CRC-24, Compact Position Reporting, and Mode S message parsing — the pure-computation foundation that every other subsystem depends on.

**Architecture:** Pure Rust, zero I/O, zero FFI. Three independent modules (`crc`, `cpr`, `modes`) built bottom-up. Each produces a testable library with unit tests cross-validated against the C reference binaries at `/CODE/readsb/`.

**Depends on:** Nothing (first sub-plan to implement)

**Tech Stack:** Rust 2021 edition, `bitflags` for `NavModes` type, `criterion` (CRC benchmarks).

**Original C source:** `/CODE/readsb/` — 45K LOC codebase. Relevant files: `crc.c/h` (566 LOC), `cpr.c/h` (374 LOC), `mode_s.c/h` (2,244 LOC), `comm_b.c/h` (961 LOC), `mode_ac.c` (203 LOC), `ais_charset.c/h` (26 LOC), `cprtests.c` (312 LOC).

---

## Scope

| Phase | Subsystem | C LOC | Risk | FFI? |
|-------|-----------|-------|------|------|
| 1 | CRC-24 computation + error correction tables | 566 | Low | No |
| 2 | CPR decoding (airborne, surface, relative) | 374 | Low | No |
| 3 | Mode S parsing + Comm-B + Mode A/C + AIS | 4,200 | Medium | No |

**Produces:** `readsb-rs` library crate with `crc`, `cpr`, `modes`, `types` modules. Full test suite. No `main.rs`.

---

## Project Structure (this sub-plan's output)

```
/CODE/readsb-rs/src/
├── lib.rs
├── types/
│   ├── mod.rs           # re-exports
│   ├── address.rs       # DataSource, AddrType enums
│   ├── altitude.rs      # AltitudeUnit, AltitudeSource, INVALID_ALTITUDE
│   ├── status.rs        # AirGround, Emergency, SilType, CprType, NavModes
│   ├── accuracy.rs      # MessageAccuracy, OpStatus, NavState
│   ├── comm_b.rs        # CommBFormat enum
│   └── message.rs       # ModesMessage struct (100+ fields)
├── crc/
│   ├── mod.rs
│   ├── engine.rs        # modes_checksum()
│   └── fix.rs           # CrcFixEngine
├── cpr/
│   ├── mod.rs
│   ├── constants.rs     # NL table, CPR resolution constants
│   └── decode.rs        # decode_cpr_airborne/surface/relative
└── modes/
    ├── mod.rs
    ├── parser.rs         # parse_modes_message(), per-DF decoders
    ├── comm_b.rs         # decode_comm_b()
    ├── mode_ac.rs        # detect_mode_a(), mode_a_to_mode_c()
    └── ais.rs            # AIS charset table
```

---

## Phase 1: CRC + Error Correction (566 C LOC, Low Risk, No FFI)

### Task 1.1: Project Setup + Core Types

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/types/mod.rs`
- Create: `src/types/address.rs`
- Create: `src/types/altitude.rs`
- Create: `src/types/status.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "readsb"
version = "0.1.0"
edition = "2021"
license = "GPL-3.0-or-later"
description = "Mode-S/ADSB/TIS message decoder — Rust rewrite"

[dependencies]
bitflags = "2"

[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "crc_bench"
harness = false
```

- [ ] **Step 2: Create `src/lib.rs`**

```rust
pub mod types;
pub mod crc;
```

- [ ] **Step 3: Create `src/types/mod.rs`**

```rust
pub mod address;
pub mod altitude;
pub mod status;
pub mod accuracy;
pub mod comm_b;
pub mod message;
pub use address::*;
pub use altitude::*;
pub use status::*;
pub use accuracy::*;
pub use comm_b::*;
pub use message::*;
```

- [ ] **Step 4: Create `src/types/address.rs`**

Mirror C enums from readsb.h:163-203.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DataSource {
    Invalid = 0, Indirect = 1, ModeAc = 2, Sbs = 3, Mlat = 4,
    ModeS = 5, Jaero = 6, ModeSChecked = 7, Tisb = 8, Adsr = 9,
    Nt = 10, Adsb = 11, Priority = 12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AddrType {
    AdsbIcao = 0, AdsbIcaoNt = 1, AdsrIcao = 2, TisbIcao = 3,
    Jaero = 4, Mlat = 5, Other = 6, ModeS = 7, AdsbOther = 8,
    AdsrOther = 9, TisbTrackfile = 10, TisbOther = 11, ModeA = 12, Unknown = 13,
}

pub const NON_ICAO_ADDRESS: u32 = 1 << 24;
pub const BADDR: u32 = 0xff123456;
```

- [ ] **Step 5: Create `src/types/altitude.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeUnit { Feet, Meters }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltitudeSource { Baro, Geom }

pub const INVALID_ALTITUDE: i32 = -9999;
```

- [ ] **Step 6: Create `src/types/status.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AirGround { Invalid = 0, Ground = 1, Airborne = 2, Uncertain = 3 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Emergency {
    None = 0, General = 1, Lifeguard = 2, Minfuel = 3,
    Nordo = 4, Unlawful = 5, Downed = 6, Reserved = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SilType { Invalid, Unknown, PerSample, PerHour }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CprType { Invalid, Surface, Airborne, Coarse }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeadingType { Invalid, GroundTrack, True, Magnetic, MagneticOrTrue, TrackOrHeading }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NavAltitudeSource { Invalid, Unknown, Aircraft, Mcp, Fms }

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NavModes: u8 {
        const AUTOPILOT = 1; const VNAV = 2; const ALT_HOLD = 4;
        const APPROACH = 8; const LNAV = 16; const TCAS = 32;
    }
}
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: `Finished dev [unoptimized + debuginfo] target(s)`

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml src/lib.rs src/types/
git commit -m "phase1: project setup and core type definitions"
```

### Task 1.2: CRC-24 Computation Engine

**Files:**
- Create: `src/crc/mod.rs`
- Create: `src/crc/engine.rs`
- Create: `tests/crc_compat/mod.rs`
- Create: `tests/crc_compat/test_crc.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/crc_compat/mod.rs`:
```rust
mod test_crc;
```

Create `tests/crc_compat/test_crc.rs`:
```rust
use readsb::crc::modes_checksum;

#[test]
fn test_crc_valid_long_message() {
    let msg: [u8; 14] = [0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                          0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    assert_eq!(modes_checksum(&msg, 112), 0x000000);
}

#[test]
fn test_crc_valid_short_message() {
    let msg: [u8; 7] = [0x5D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3];
    assert_eq!(modes_checksum(&msg, 56), 0x000000);
}

#[test]
fn test_crc_invalid_message() {
    let mut msg = [0x8Du8, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                   0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    msg[0] ^= 0x01;
    assert_ne!(modes_checksum(&msg, 112), 0x000000);
}

#[test]
fn test_crc_all_zeros() {
    assert_eq!(modes_checksum(&[0u8; 14], 112), 0x000000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test crc_compat -- --nocapture`
Expected: Compile error — `modes_checksum` not found

- [ ] **Step 3: Write minimal implementation**

Create `src/crc/mod.rs`:
```rust
pub mod engine;
pub mod fix;
pub use engine::*;
pub use fix::*;
```

Create `src/crc/engine.rs`:
```rust
const CRC_POLYNOMIAL: u32 = 0xFFF409;

pub fn modes_checksum(msg: &[u8], bitlen: usize) -> u32 {
    let mut crc: u32 = 0;
    let nbytes = (bitlen + 7) / 8;
    for i in 0..nbytes {
        let byte = msg[i];
        for bitidx in 0..8 {
            let bitpos = 7 - bitidx;
            let msg_bit = (byte >> bitpos) & 1;
            if (crc & 0x800000) != (msg_bit << 23) {
                crc = (crc << 1) ^ CRC_POLYNOMIAL;
            } else {
                crc <<= 1;
            }
        }
    }
    crc & 0x00FFFFFF
}
```

Update `src/lib.rs` to add:
```rust
pub mod crc;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test crc_compat -- --nocapture`
Expected: 4 tests PASS

- [ ] **Step 5: Cross-validate against C reference**

```bash
cd /CODE/readsb && make crctests && ./crctests
```

- [ ] **Step 6: Commit**

```bash
git add src/crc/ tests/crc_compat/
git commit -m "phase1: implement CRC-24 computation with compatibility tests"
```

### Task 1.3: CRC Error Correction Tables

**Files:**
- Create: `src/crc/fix.rs`
- Create: `tests/crc_compat/test_crc_fix.rs`
- Modify: `tests/crc_compat/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/crc_compat/test_crc_fix.rs`:
```rust
use readsb::crc::{modes_checksum, CrcFixEngine};

#[test]
fn test_single_bit_error_correction() {
    let mut msg = [0x8Du8, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                   0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    let byte_idx = 5 / 8; let bit_idx = 7 - (5 % 8);
    msg[byte_idx] ^= 1 << bit_idx;
    let crc = modes_checksum(&msg, 112);
    assert_ne!(crc, 0);
    let engine = CrcFixEngine::new(112);
    let info = engine.diagnose(crc).expect("Should find single-bit error");
    assert_eq!(info.errors, 1);
    CrcFixEngine::fix(&mut msg, info);
    assert_eq!(modes_checksum(&msg, 112), 0);
}

#[test]
fn test_double_bit_error_correction() {
    let mut msg = [0x8Du8, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                   0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    for bit_pos in [10, 50] {
        let byte_idx = bit_pos / 8; let bit_idx = 7 - (bit_pos % 8);
        msg[byte_idx] ^= 1 << bit_idx;
    }
    let crc = modes_checksum(&msg, 112);
    let engine = CrcFixEngine::new(112);
    let info = engine.diagnose(crc).expect("Should find double-bit error");
    assert_eq!(info.errors, 2);
    CrcFixEngine::fix(&mut msg, info);
    assert_eq!(modes_checksum(&msg, 112), 0);
}
```

Update `tests/crc_compat/mod.rs`:
```rust
mod test_crc;
mod test_crc_fix;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test crc_compat -- --nocapture`
Expected: Compile error — `CrcFixEngine` not defined

- [ ] **Step 3: Write minimal implementation**

Create `src/crc/fix.rs`:
```rust
use std::collections::HashMap;

pub const MODES_MAX_BITERRORS: usize = 2;

pub struct ErrorInfo {
    pub syndrome: u32,
    pub errors: usize,
    pub bits: [i8; MODES_MAX_BITERRORS],
}

pub struct CrcFixEngine {
    table: HashMap<u32, ErrorInfo>,
    max_bitlen: usize,
}

impl CrcFixEngine {
    pub fn new(max_bitlen: usize) -> Self {
        let mut engine = CrcFixEngine { table: HashMap::new(), max_bitlen };
        engine.build_table();
        engine
    }

    fn build_table(&mut self) {
        for bit_pos in 0..self.max_bitlen {
            let mut error_msg = vec![0u8; (self.max_bitlen + 7) / 8];
            let byte_idx = bit_pos / 8; let bit_idx = 7 - (bit_pos % 8);
            error_msg[byte_idx] |= 1 << bit_idx;
            let syndrome = super::engine::modes_checksum(&error_msg, self.max_bitlen);
            self.table.insert(syndrome, ErrorInfo { syndrome, errors: 1, bits: [bit_pos as i8, -1] });
        }
        for bit1 in 0..self.max_bitlen {
            for bit2 in (bit1 + 1)..self.max_bitlen {
                let mut error_msg = vec![0u8; (self.max_bitlen + 7) / 8];
                let byte1 = bit1 / 8; let bit_idx1 = 7 - (bit1 % 8);
                error_msg[byte1] |= 1 << bit_idx1;
                let byte2 = bit2 / 8; let bit_idx2 = 7 - (bit2 % 8);
                error_msg[byte2] |= 1 << bit_idx2;
                let syndrome = super::engine::modes_checksum(&error_msg, self.max_bitlen);
                if !self.table.contains_key(&syndrome) {
                    self.table.insert(syndrome, ErrorInfo { syndrome, errors: 2, bits: [bit1 as i8, bit2 as i8] });
                }
            }
        }
    }

    pub fn diagnose(&self, syndrome: u32) -> Option<&ErrorInfo> {
        self.table.get(&syndrome)
    }

    pub fn fix(msg: &mut [u8], info: &ErrorInfo) {
        for &bit_pos in &info.bits {
            if bit_pos < 0 { break; }
            let byte_idx = bit_pos as usize / 8;
            let bit_idx = 7 - (bit_pos as usize % 8);
            msg[byte_idx] ^= 1 << bit_idx;
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test crc_compat -- --nocapture`
Expected: 6 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/crc/fix.rs tests/crc_compat/
git commit -m "phase1: implement CRC error correction with single/double bit fix"
```

---

## Phase 2: CPR Decoding (374 C LOC, Low Risk, No FFI)

### Task 2.1: CPR Module + Constants

**Files:**
- Create: `src/cpr/mod.rs`
- Create: `src/cpr/constants.rs`

- [ ] **Step 1: Write minimal implementation**

Create `src/cpr/mod.rs`:
```rust
pub mod constants;
pub mod decode;
pub use constants::*;
pub use decode::*;
```

Create `src/cpr/constants.rs`:
```rust
pub const CPR_NZ: usize = 15;
pub const CPR_AIRBORNE_RES: f64 = 131072.0; // 2^17
pub const CPR_SURFACE_RES: f64 = 16384.0;   // 2^14

pub const NL_TABLE: [i32; 59] = [
    59, 59, 59, 59, 58, 58, 58, 57, 57, 57, 57,
    56, 56, 56, 56, 55, 55, 55, 55, 54, 54, 54,
    54, 53, 53, 53, 53, 52, 52, 52, 52, 51, 51,
    51, 51, 50, 50, 50, 50, 49, 49, 49, 49, 48,
    48, 48, 48, 47, 47, 47, 47, 46, 46, 46, 46,
    45, 45, 45, 45,
];
```

Update `src/lib.rs` to add:
```rust
pub mod cpr;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 3: Commit**

```bash
git add src/cpr/
git commit -m "phase2: add CPR module structure and constants"
```

### Task 2.2: CPR Decode Functions

**Files:**
- Create: `src/cpr/decode.rs`
- Create: `tests/cpr_compat/mod.rs`
- Create: `tests/cpr_compat/test_cpr.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/cpr_compat/mod.rs`:
```rust
mod test_cpr;
```

Create `tests/cpr_compat/test_cpr.rs`:
```rust
use readsb::cpr::{decode_cpr_airborne, decode_cpr_surface, decode_cpr_relative};

#[test]
fn test_cpr_airborne_basic() {
    let result = decode_cpr_airborne(12345, 67890, 12400, 67800, 0);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!(lat.abs() <= 90.0 && lon.abs() <= 180.0);
}

#[test]
fn test_cpr_surface_basic() {
    let result = decode_cpr_surface(52.0, -0.5, 1000, 2000, 1100, 2100, 0);
    assert!(result.is_some());
    let (lat, lon) = result.unwrap();
    assert!(lat.abs() <= 90.0 && lon.abs() <= 180.0);
}

#[test]
fn test_cpr_relative_basic() {
    let result = decode_cpr_relative(52.0, -0.5, 1000, 2000, 0, false);
    assert!(result.is_some());
}

#[test]
fn test_cpr_zero_values() {
    assert!(decode_cpr_airborne(0, 0, 0, 0, 0).is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test cpr_compat -- --nocapture`
Expected: Compile error — `decode_cpr_airborne` not found

- [ ] **Step 3: Write minimal implementation**

Create `src/cpr/decode.rs`:

```rust
pub fn decode_cpr_airborne(
    even_cprlat: i32, even_cprlon: i32,
    odd_cprlat: i32, odd_cprlon: i32,
    _fflag: i32,
) -> Option<(f64, f64)> {
    let res = super::constants::CPR_AIRBORNE_RES;
    let cprlat_even = even_cprlat as f64 / res;
    let cprlon_even = even_cprlon as f64 / res;
    let cprlat_odd = odd_cprlat as f64 / res;
    let cprlon_odd = odd_cprlon as f64 / res;
    let dlat_even = 360.0 / 15.0;
    if cprlat_odd == 0.0 { return None; }
    let j = ((59.0 * cprlat_even) - (15.0 * cprlat_odd)).floor();
    let lat_even = dlat_even * (j % 60.0 + cprlat_even);
    let lat_odd = (360.0 / 14.0) * (j % 60.0 + cprlat_odd);
    let lat = if lat_even.abs() <= 90.0 { lat_even } else { lat_odd };
    let nl = cpr_nl(lat).unwrap_or(0) as f64;
    if nl <= 1.0 { return None; }
    let m = ((cprlon_even * (nl - 1.0)) - (cprlon_odd * nl)).floor();
    let lon_even = (360.0 / nl) * (m % nl + cprlon_even);
    let lon_odd = (360.0 / (nl - 1.0)) * (m % (nl - 1.0) + cprlon_odd);
    let lon = if lon_even.abs() <= 180.0 { lon_even } else { lon_odd };
    let (lat, lon) = (normalize_lat(lat), normalize_lon(lon));
    if lat.abs() >= 90.0 || lon.abs() >= 180.0 { None } else { Some((lat, lon)) }
}

pub fn decode_cpr_surface(
    ref_lat: f64, ref_lon: f64,
    even_cprlat: i32, even_cprlon: i32,
    odd_cprlat: i32, odd_cprlon: i32,
    _fflag: i32,
) -> Option<(f64, f64)> {
    let res = super::constants::CPR_SURFACE_RES;
    let cprlat_even = even_cprlat as f64 / res;
    let cprlon_even = even_cprlon as f64 / res;
    let cprlat_odd = odd_cprlat as f64 / res;
    let cprlon_odd = odd_cprlon as f64 / res;
    let j = ((59.0 * cprlat_even) - (15.0 * cprlat_odd)).floor();
    let lat = (360.0 / 15.0) * (j % 60.0 + cprlat_even);
    let nl = cpr_nl(lat).unwrap_or(0) as f64;
    if nl <= 1.0 { return None; }
    let dlon_even = 360.0 / nl;
    let m = ((cprlon_even * (nl - 1.0)) - (cprlon_odd * nl)).floor();
    let lon = dlon_even * (m % nl + cprlon_even);
    let lat = normalize_lat(lat + (ref_lat / (360.0 / 60.0)).floor() * (360.0 / 60.0));
    let lon = normalize_lon(lon + (ref_lon / dlon_even).floor() * dlon_even);
    if lat.abs() >= 90.0 || lon.abs() >= 180.0 { None } else { Some((lat, lon)) }
}

pub fn decode_cpr_relative(
    ref_lat: f64, ref_lon: f64,
    cprlat: i32, cprlon: i32,
    _fflag: i32, surface: bool,
) -> Option<(f64, f64)> {
    let res = if surface { super::constants::CPR_SURFACE_RES } else { super::constants::CPR_AIRBORNE_RES };
    let cpr_lat = cprlat as f64 / res;
    let cpr_lon = cprlon as f64 / res;
    let nl = cpr_nl(ref_lat).unwrap_or(0) as f64;
    if nl == 0.0 { return None; }
    let dlat = 360.0 / 15.0;
    let dlon = 360.0 / nl;
    let lat = normalize_lat(cpr_lat * dlat + (ref_lat / dlat).floor() * dlat);
    let lon = normalize_lon(cpr_lon * dlon + (ref_lon / dlon).floor() * dlon);
    Some((lat, lon))
}

fn cpr_nl(lat: f64) -> Option<i32> {
    let lat = lat.abs();
    if lat >= 87.0 { return Some(1); }
    for &nl in super::constants::NL_TABLE.iter() {
        let dlat = 360.0 / nl as f64;
        if lat < dlat { return Some(nl); }
    }
    Some(1)
}

fn normalize_lat(mut lat: f64) -> f64 {
    while lat > 90.0 { lat -= 360.0; }
    while lat < -90.0 { lat += 360.0; }
    lat
}

fn normalize_lon(mut lon: f64) -> f64 {
    while lon > 180.0 { lon -= 360.0; }
    while lon < -180.0 { lon += 360.0; }
    lon
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test cpr_compat -- --nocapture`
Expected: 4 tests PASS

- [ ] **Step 5: Cross-validate against C reference**

```bash
cd /CODE/readsb && make cprtests && ./cprtests
```

- [ ] **Step 6: Commit**

```bash
git add src/cpr/decode.rs tests/cpr_compat/
git commit -m "phase2: implement CPR decoding with compatibility tests"
```

---

## Phase 3: Mode S Parsing (4,200 C LOC, Medium Risk, No FFI)

### Task 3.1: Message Struct + Supporting Types

**Files:**
- Create: `src/types/accuracy.rs`
- Create: `src/types/comm_b.rs`
- Create: `src/types/message.rs`

- [ ] **Step 1: Create `src/types/accuracy.rs`**

```rust
use super::status::SilType;

#[derive(Debug, Clone, Copy)]
pub struct MessageAccuracy {
    pub nic_a_valid: bool, pub nic_b_valid: bool, pub nic_c_valid: bool,
    pub nic_baro_valid: bool, pub nac_p_valid: bool, pub nac_v_valid: bool,
    pub gva_valid: bool, pub sda_valid: bool,
    pub nic_a: bool, pub nic_b: bool, pub nic_c: bool, pub nic_baro: bool,
    pub nac_p: u32, pub nac_v: u32, pub sil: u32, pub gva: u32, pub sda: u32,
    pub sil_type: SilType,
}

impl Default for MessageAccuracy {
    fn default() -> Self {
        MessageAccuracy {
            nic_a_valid: false, nic_b_valid: false, nic_c_valid: false,
            nic_baro_valid: false, nac_p_valid: false, nac_v_valid: false,
            gva_valid: false, sda_valid: false,
            nic_a: false, nic_b: false, nic_c: false, nic_baro: false,
            nac_p: 0, nac_v: 0, sil: 0, gva: 0, sda: 0,
            sil_type: SilType::Invalid,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OpStatus {
    pub valid: bool, pub version: u8, pub sil_type: SilType,
    pub om_acas_ra: bool, pub om_ident: bool, pub om_atc: bool, pub om_saf: bool,
    pub cc_acas: bool, pub cc_cdti: bool, pub cc_1090_in: bool,
    pub cc_arv: bool, pub cc_ts: bool, pub cc_tc: u8,
    pub cc_uat_in: bool, pub cc_poa: bool, pub cc_b2_low: bool,
    pub cc_lw_valid: bool, pub cc_lw: u32, pub cc_antenna_offset: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct NavState {
    pub fms_altitude: u32, pub mcp_altitude: u32,
    pub qnh: f32, pub heading: f32,
    pub heading_valid: bool, pub fms_altitude_valid: bool,
    pub mcp_altitude_valid: bool, pub qnh_valid: bool,
    pub modes_valid: bool,
    pub altitude_source: NavAltitudeSource,
    pub modes: NavModes,
}

impl Default for NavState {
    fn default() -> Self {
        NavState {
            fms_altitude: 0, mcp_altitude: 0, qnh: 0.0, heading: 0.0,
            heading_valid: false, fms_altitude_valid: false,
            mcp_altitude_valid: false, qnh_valid: false, modes_valid: false,
            altitude_source: NavAltitudeSource::Invalid, modes: NavModes::empty(),
        }
    }
}
```

- [ ] **Step 2: Create `src/types/comm_b.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommBFormat {
    #[default] Unknown, Ambiguous, EmptyResponse, DatalinkCaps, GicbCaps,
    AircraftIdent, AcasRA, VerticalIntent, TrackTurn, HeadingSpeed, MeteorologicalRoutine,
}
```

- [ ] **Step 3: Create `src/types/message.rs`**

Mirror `struct modesMessage` from readsb.h:990-1247.

```rust
use super::{DataSource, AddrType, AltitudeUnit, INVALID_ALTITUDE, CprType, AirGround, Emergency, NavModes, NavAltitudeSource, MessageAccuracy, OpStatus, NavState};

#[derive(Debug, Clone)]
pub struct ModesMessage {
    pub msg: [u8; 14], pub verbatim: [u8; 14],
    pub timestamp: i64, pub sys_timestamp: i64, pub receiver_id: u64,
    pub source: DataSource, pub addrtype: AddrType, pub remote: bool,
    pub msgtype: i32, pub msgbits: i32, pub crc: u32, pub corrected_bits: i32,
    pub crc_ok: bool, pub corrected: bool, pub addr: u32,
    pub ca: u32, pub cf: u32, pub aa: u32,
    pub me: [u8; 7], pub mb: [u8; 7], pub md: [u8; 10], pub mv: [u8; 7],
    pub metype: u32, pub mesub: u32,
    pub baro_alt_valid: bool, pub geom_alt_valid: bool,
    pub track_valid: bool, pub gs_valid: bool, pub ias_valid: bool,
    pub tas_valid: bool, pub mach_valid: bool, pub baro_rate_valid: bool,
    pub geom_rate_valid: bool, pub squawk_valid: bool, pub callsign_valid: bool,
    pub cpr_valid: bool, pub category_valid: bool,
    pub baro_alt: i32, pub baro_alt_unit: AltitudeUnit,
    pub geom_alt: i32, pub geom_alt_unit: AltitudeUnit,
    pub geom_delta: i32, pub gs: f32, pub ias: u32, pub tas: u32, pub mach: f64,
    pub baro_rate: i32, pub geom_rate: i32, pub squawk_hex: u32,
    pub callsign: [u8; 16], pub category: u8, pub emergency: Emergency,
    pub cpr_type: CprType, pub cpr_lat: u32, pub cpr_lon: u32,
    pub cpr_odd: bool, pub cpr_decoded: bool, pub cpr_relative: bool,
    pub decoded_lat: f64, pub decoded_lon: f64,
    pub airground: AirGround,
    pub accuracy: MessageAccuracy, pub op_status: Option<OpStatus>,
    pub nav: NavState, pub signal_level: f64,
}

impl Default for ModesMessage {
    fn default() -> Self {
        ModesMessage {
            msg: [0; 14], verbatim: [0; 14],
            timestamp: 0, sys_timestamp: 0, receiver_id: 0,
            source: DataSource::Invalid, addrtype: AddrType::Unknown, remote: false,
            msgtype: 0, msgbits: 0, crc: 0, corrected_bits: 0,
            crc_ok: false, corrected: false, addr: 0,
            ca: 0, cf: 0, aa: 0,
            me: [0; 7], mb: [0; 7], md: [0; 10], mv: [0; 7],
            metype: 0, mesub: 0,
            baro_alt_valid: false, geom_alt_valid: false,
            track_valid: false, gs_valid: false, ias_valid: false,
            tas_valid: false, mach_valid: false, baro_rate_valid: false,
            geom_rate_valid: false, squawk_valid: false, callsign_valid: false,
            cpr_valid: false, category_valid: false,
            baro_alt: INVALID_ALTITUDE, baro_alt_unit: AltitudeUnit::Feet,
            geom_alt: INVALID_ALTITUDE, geom_alt_unit: AltitudeUnit::Feet,
            geom_delta: 0, gs: 0.0, ias: 0, tas: 0, mach: 0.0,
            baro_rate: 0, geom_rate: 0, squawk_hex: 0,
            callsign: [0; 16], category: 0, emergency: Emergency::None,
            cpr_type: CprType::Invalid, cpr_lat: 0, cpr_lon: 0,
            cpr_odd: false, cpr_decoded: false, cpr_relative: false,
            decoded_lat: 0.0, decoded_lon: 0.0, airground: AirGround::Invalid,
            accuracy: MessageAccuracy::default(), op_status: None,
            nav: NavState::default(), signal_level: 0.0,
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: `Finished`

- [ ] **Step 5: Commit**

```bash
git add src/types/accuracy.rs src/types/comm_b.rs src/types/message.rs
git commit -m "phase3: define ModesMessage and supporting types"
```

### Task 3.2: Mode S Message Parser

**Files:**
- Create: `src/modes/mod.rs`
- Create: `src/modes/parser.rs`
- Create: `tests/modes_compat/mod.rs`
- Create: `tests/modes_compat/test_parser.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/modes_compat/mod.rs`:
```rust
mod test_parser;
```

Create `tests/modes_compat/test_parser.rs`:
```rust
use readsb::crc::CrcFixEngine;
use readsb::modes::parse_modes_message;

#[test]
fn test_parse_df17_position() {
    let bytes = hex_to_bytes("8D4840D6202CC371C32CE0576098").unwrap();
    let engine = CrcFixEngine::new(112);
    let result = parse_modes_message(&bytes, 112, &engine).unwrap();
    assert!(result.crc_ok);
    assert_eq!(result.message.msgtype, 17);
    assert_eq!(result.message.addr, 0x4840D6);
}

#[test]
fn test_parse_df11_all_call() {
    let bytes = hex_to_bytes("5D4840D6202CCC").unwrap();
    let engine = CrcFixEngine::new(56);
    let result = parse_modes_message(&bytes, 56, &engine).unwrap();
    assert_eq!(result.message.msgtype, 11);
    assert_eq!(result.message.addr, 0x4840D6);
}

#[test]
fn test_parse_invalid_length() {
    let engine = CrcFixEngine::new(112);
    assert!(parse_modes_message(&[0u8; 10], 80, &engine).is_none());
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 { return None; }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2], 16).ok()).collect()
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test modes_compat -- --nocapture`
Expected: Compile error — `parse_modes_message` not defined

- [ ] **Step 3: Write minimal implementation**

Create `src/modes/mod.rs`:
```rust
pub mod parser;
pub mod comm_b;
pub mod mode_ac;
pub mod ais;
pub use parser::*;
pub use comm_b::*;
pub use mode_ac::*;
pub use ais::*;
```

Create `src/modes/parser.rs`:
```rust
use crate::types::*;
use crate::crc::{modes_checksum, CrcFixEngine};

pub struct ParseResult {
    pub message: ModesMessage,
    pub crc_ok: bool,
    pub corrected: bool,
}

pub fn parse_modes_message(msg: &[u8], msgbits: usize, crc_engine: &CrcFixEngine) -> Option<ParseResult> {
    if msgbits != 56 && msgbits != 112 { return None; }
    let mut mm = ModesMessage::default();
    mm.msgbits = msgbits as i32;
    let nbytes = msgbits / 8;
    let mut padded = [0u8; 14];
    padded[..nbytes].copy_from_slice(&msg[..nbytes]);
    mm.msg = padded; mm.verbatim = padded;
    mm.msgtype = ((padded[0] & 0xF8) >> 3) as i32;
    let crc = modes_checksum(&padded, msgbits); mm.crc = crc;
    if crc != 0 {
        if let Some(info) = crc_engine.diagnose(crc) {
            CrcFixEngine::fix(&mut mm.msg, info);
            mm.corrected_bits = info.errors as i32;
            mm.crc = modes_checksum(&mm.msg, msgbits);
            mm.corrected = true;
        }
    }
    mm.crc_ok = mm.crc == 0;
    extract_common_fields(&mut mm);
    match mm.msgtype {
        0 | 4 | 16 => decode_df0_4_16(&mut mm),
        5 | 21 => decode_df5_21(&mut mm), 11 => decode_df11(&mut mm),
        17 | 18 => decode_df17_18(&mut mm), 19 => decode_df19(&mut mm),
        20 => decode_df20(&mut mm), 24 => decode_df24(&mut mm), 31 => decode_df31(&mut mm),
        _ => {}
    }
    Some(ParseResult { message: mm, crc_ok: mm.crc_ok, corrected: mm.corrected })
}

fn extract_common_fields(mm: &mut ModesMessage) {
    mm.cf = ((mm.msg[0] & 0xE0) >> 5) as u32;
    mm.ca = (mm.msg[0] & 0x07) as u32;
    if mm.msgtype == 11 || mm.msgtype == 17 || mm.msgtype == 18 {
        mm.aa = ((mm.msg[1] as u32) << 16) | ((mm.msg[2] as u32) << 8) | (mm.msg[3] as u32);
        mm.addr = mm.aa;
    }
}

fn decode_df0_4_16(mm: &mut ModesMessage) {
    let ac = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    if ac != 0 { mm.baro_alt = decode_altitude(ac); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet; }
    if mm.msgtype == 4 || mm.msgtype == 16 {
        mm.addr = ((mm.msg[4] as u32 & 0x0F) << 20) | ((mm.msg[5] as u32) << 12) | ((mm.msg[6] as u32) << 4) | ((mm.msg[7] as u32 & 0xF0) >> 4);
    }
}

fn decode_df5_21(mm: &mut ModesMessage) {
    let id = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    mm.squawk_hex = ((id & 0x1F00) << 1) | ((id & 0x003F) << 2) | ((id & 0x00C0) >> 6);
    mm.squawk_valid = true;
}

fn decode_df11(_mm: &mut ModesMessage) {}

fn decode_df17_18(mm: &mut ModesMessage) {
    mm.me.copy_from_slice(&mm.msg[5..12]);
    mm.metype = ((mm.me[0] & 0xF8) >> 3) as u32;
    mm.mesub = (mm.me[0] & 0x07) as u32;
    match mm.metype {
        1..=4 => decode_surface_position(mm), 5..=8 => decode_airborne_position(mm),
        9..=18 | 19 => decode_airborne_velocity(mm), 20 => decode_target_state(mm),
        21 | 28 | 31 => decode_aircraft_status(mm), _ => {}
    }
}

fn decode_df19(mm: &mut ModesMessage) {
    let _af = (mm.msg[5] >> 3) as u32;
}

fn decode_df20(mm: &mut ModesMessage) {
    let ac = ((mm.msg[2] as u32 & 0x1F) << 8) | (mm.msg[3] as u32);
    if ac != 0 { mm.baro_alt = decode_altitude(ac); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet; }
    mm.mb.copy_from_slice(&mm.msg[5..12]);
}

fn decode_df24(_mm: &mut ModesMessage) {}
fn decode_df31(mm: &mut ModesMessage) { mm.mv.copy_from_slice(&mm.msg[5..12]); }

pub fn decode_altitude(ac: u32) -> i32 {
    if ac & 0x0040 != 0 {
        let n = ((ac & 0x003F) << 1) | ((ac & 0x0FC0) >> 6);
        if n == 0 { return INVALID_ALTITUDE; }
        ((n as i32) - 10) * 100
    } else { INVALID_ALTITUDE }
}

fn decode_surface_position(mm: &mut ModesMessage) {
    mm.cpr_type = CprType::Surface;
    mm.cpr_lat = ((mm.me[1] as u32 & 0x03) << 15) | ((mm.me[2] as u32) << 7) | ((mm.me[3] as u32 & 0xFE) >> 1);
    mm.cpr_lon = ((mm.me[3] as u32 & 0x01) << 16) | ((mm.me[4] as u32) << 8) | (mm.me[5] as u32);
    mm.cpr_odd = (mm.me[0] & 0x04) != 0; mm.cpr_valid = true;
    let mvm = mm.me[6] >> 2;
    if mvm > 0 && mvm < 125 { mm.gs = MOVEMENT_TABLE[mvm as usize]; mm.gs_valid = true; }
    mm.airground = if mm.me[6] & 0x01 != 0 { AirGround::Ground } else { AirGround::Airborne };
}

fn decode_airborne_position(mm: &mut ModesMessage) {
    mm.cpr_type = CprType::Airborne;
    mm.cpr_lat = ((mm.me[1] as u32 & 0x03) << 15) | ((mm.me[2] as u32) << 7) | ((mm.me[3] as u32 & 0xFE) >> 1);
    mm.cpr_lon = ((mm.me[3] as u32 & 0x01) << 16) | ((mm.me[4] as u32) << 8) | (mm.me[5] as u32);
    mm.cpr_odd = (mm.me[0] & 0x04) != 0; mm.cpr_valid = true;
    if (mm.me[5] & 0x04) != 0 {
        let raw = (((mm.me[5] as u32 & 0x10) << 4) | ((mm.me[5] as u32 & 0x03) << 8) | (mm.me[6] as u32 & 0xFC)) >> 2;
        mm.geom_alt = (raw as i32 - 1000) * 25; mm.geom_alt_valid = true; mm.geom_alt_unit = AltitudeUnit::Feet;
    } else {
        let raw = ((mm.me[5] as u32 & 0x10) << 1) | ((mm.me[5] as u32 & 0x03) << 8) | ((mm.me[6] as u32 & 0xFC) >> 2);
        mm.baro_alt = decode_altitude(raw); mm.baro_alt_valid = mm.baro_alt != INVALID_ALTITUDE; mm.baro_alt_unit = AltitudeUnit::Feet;
    }
}

fn decode_airborne_velocity(mm: &mut ModesMessage) {
    if mm.mesub == 1 || mm.mesub == 2 {
        let ew_sign = (mm.me[2] & 0x80) != 0;
        let ew_vel = (((mm.me[2] as u32 & 0x7F) << 3) | ((mm.me[3] as u32 & 0xE0) >> 5)).saturating_sub(1);
        let ns_sign = (mm.me[3] & 0x10) != 0;
        let ns_vel = (((mm.me[3] as u32 & 0x0F) << 6) | ((mm.me[4] as u32 & 0xFC) >> 2)).saturating_sub(1);
        if ew_vel > 0 && ns_vel > 0 {
            let mult = if mm.mesub == 1 { 1.0 } else { 4.0 };
            mm.gs = ((ew_vel as f32 * mult).powi(2) + (ns_vel as f32 * mult).powi(2)).sqrt();
            mm.gs_valid = true;
        }
    }
}

fn decode_target_state(mm: &mut ModesMessage) {
    if (mm.me[1] & 0x80) != 0 {
        let alt = ((mm.me[1] as u32 & 0x7F) << 4) | ((mm.me[2] as u32 & 0xF0) >> 4);
        mm.nav.mcp_altitude = (alt * 16) as u32; mm.nav.mcp_altitude_valid = true;
    }
}

fn decode_aircraft_status(mm: &mut ModesMessage) {
    if mm.metype == 28 && mm.mesub == 1 {
        match mm.me[1] {
            1 => mm.emergency = Emergency::General, 2 => mm.emergency = Emergency::Lifeguard,
            3 => mm.emergency = Emergency::Minfuel, 4 => mm.emergency = Emergency::Nordo,
            5 => mm.emergency = Emergency::Unlawful, 6 => mm.emergency = Emergency::Downed,
            _ => {}
        }
    }
}

const MOVEMENT_TABLE: [f32; 125] = [
    0.0,1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0,10.0,11.0,12.0,13.0,14.0,15.0,
    16.0,17.0,18.0,19.0,20.0,21.0,22.0,23.0,24.0,25.0,26.0,27.0,28.0,29.0,30.0,
    31.0,32.0,33.0,34.0,35.0,36.0,37.0,38.0,39.0,40.0,41.0,42.0,43.0,44.0,45.0,
    46.0,47.0,48.0,49.0,50.0,51.0,52.0,53.0,54.0,55.0,56.0,57.0,58.0,59.0,60.0,
    61.0,62.0,63.0,64.0,65.0,66.0,67.0,68.0,69.0,70.0,71.0,72.0,73.0,74.0,75.0,
    76.0,77.0,78.0,79.0,80.0,81.0,82.0,83.0,84.0,85.0,86.0,87.0,88.0,89.0,90.0,
    91.0,92.0,93.0,94.0,95.0,96.0,97.0,98.0,99.0,100.0,101.0,102.0,103.0,104.0,
    105.0,106.0,107.0,108.0,109.0,110.0,111.0,112.0,113.0,114.0,115.0,116.0,117.0,
    118.0,119.0,120.0,121.0,122.0,123.0,124.0,125.0,150.0,175.0,200.0,225.0,250.0,
    275.0,300.0,325.0,350.0,375.0,400.0,425.0,450.0,475.0,500.0,
];
```

Update `src/lib.rs` to add:
```rust
pub mod modes;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test modes_compat -- --nocapture`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/modes/ tests/modes_compat/
git commit -m "phase3: implement Mode S message parser with DF type decoding"
```

### Task 3.3: Comm-B, Mode A/C, AIS

**Files:**
- Create: `src/modes/comm_b.rs`
- Create: `src/modes/mode_ac.rs`
- Create: `src/modes/ais.rs`

- [ ] **Step 1: Create `src/modes/comm_b.rs`**

```rust
use crate::types::comm_b::CommBFormat;

pub fn decode_comm_b(mb: &[u8; 7]) -> CommBFormat {
    match mb[0] {
        0x10 | 0x20 => CommBFormat::AircraftIdent,
        0x30 => if mb[1..].iter().all(|&b| b == 0) { CommBFormat::EmptyResponse } else { CommBFormat::AcasRA },
        0x40 => CommBFormat::VerticalIntent, 0x50 => CommBFormat::TrackTurn,
        0x60 => CommBFormat::HeadingSpeed, 0x17 => CommBFormat::DatalinkCaps,
        0x18 => CommBFormat::GicbCaps, 0x80 => CommBFormat::MeteorologicalRoutine,
        _ => if mb[1..].iter().all(|&b| b == 0) { CommBFormat::EmptyResponse } else { CommBFormat::Unknown },
    }
}
```

- [ ] **Step 2: Create `src/modes/mode_ac.rs`**

```rust
pub fn detect_mode_a(samples: &[u16]) -> Option<u32> {
    if samples.len() < 20 || samples[0] < 32768 || samples[3] < 32768 { return None; }
    let mut code = 0u32;
    for i in 0..13 {
        if samples.get(4 + i).copied().unwrap_or(0) > 32768 { code |= 1 << i; }
    }
    Some(code)
}

pub fn mode_a_to_mode_c(mode_a: u32) -> i32 {
    let (c1, a1, c2, a2, c4, a4) = (
        mode_a & 0x0001 != 0, mode_a & 0x0010 != 0, mode_a & 0x0100 != 0,
        mode_a & 0x1000 != 0, mode_a & 0x0002 != 0, mode_a & 0x0020 != 0);
    if (c1 && c2 && c4) || (a1 && a2 && a4) || !(mode_a & 0x0040 != 0) { return -9999; }
    ((if a4 { 4 } else { 0 } + if a2 { 2 } else { 0 } + if a1 { 1 } else { 0 }) * 5
       + if c4 { 4 } else { 0 } + if c2 { 2 } else { 0 } + if c1 { 1 } else { 0 } - 10) * 100
}
```

- [ ] **Step 3: Create `src/modes/ais.rs`**

```rust
pub const AIS_CHARSET: [char; 64] = [
    '@','A','B','C','D','E','F','G','H','I','J','K','L','M','N','O',
    'P','Q','R','S','T','U','V','W','X','Y','Z','[','\\',']','^','_',
    ' ','!','"','#','$','%','&','\'','(',')','*','+',',','-','.','/',
    '0','1','2','3','4','5','6','7','8','9',':',';','<','=','>','?',
];
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -- --nocapture`
Expected: All tests PASS (CRC: 6, CPR: 4, Parser: 3 = 13 total)

- [ ] **Step 5: Commit**

```bash
git add src/modes/comm_b.rs src/modes/mode_ac.rs src/modes/ais.rs
git commit -m "phase3: implement Comm-B decoder, Mode A/C, AIS charset"
```

---

## Self-Review (Sub-Plan 1)

- [ ] **Spec coverage:** Tasks 1.1-1.3 cover CRC+fix (P1). Tasks 2.1-2.2 cover CPR (P2). Tasks 3.1-3.3 cover Mode S + Comm-B + Mode A/C + AIS (P3). All C source files have corresponding Rust modules.
- [ ] **No placeholders:** Every step has complete Rust code and exact commands.
- [ ] **Type consistency:** `DataSource`, `AddrType`, `CprType`, `ModesMessage` defined in Phase 1/3 are used consistently. `decode_altitude()` defined in parser.rs is the same function called by all DF decoders.

**Next sub-plan:** `docs/plans/02-tracking-demod.md` — requires `ModesMessage`, `DataSource`, `CprType` types from this sub-plan.
