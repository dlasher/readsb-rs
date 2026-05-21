use std::collections::HashMap;
use std::time::Instant;

use crate::console::formatter::{format_high, format_low, format_max, format_medium};
use crate::console::state::{diff_snapshots, snapshot_from_message, AircraftSnapshot, ConsoleLevel};
use crate::console::stats_accumulator::StatsAccumulator;
use crate::types::ModesMessage;

pub struct Outputter {
    level: ConsoleLevel,
    medium_interval_secs: u64,
    last_low_report: Instant,
    last_medium_report: Instant,
    last_aircraft_states: HashMap<u32, AircraftSnapshot>,
    current_aircraft_states: HashMap<u32, AircraftSnapshot>,
    start_time: Instant,
    stats: StatsAccumulator,
}

impl Outputter {
    pub fn new(level: ConsoleLevel, medium_interval_secs: u64) -> Self {
        let now = Instant::now();
        Outputter {
            level,
            medium_interval_secs,
            last_low_report: now,
            last_medium_report: now,
            last_aircraft_states: HashMap::new(),
            current_aircraft_states: HashMap::new(),
            start_time: now,
            stats: StatsAccumulator::new(),
        }
    }

    pub fn set_level(&mut self, level: ConsoleLevel) {
        self.level = level;
    }

    pub fn level(&self) -> ConsoleLevel {
        self.level
    }

    pub fn feed(&mut self, msg: &ModesMessage, now: Instant) -> Vec<String> {
        let mut output = Vec::new();

        // record message in stats accumulator
        self.stats.record_message(
            msg.msgtype as u8,
            msg.crc_ok,
            msg.corrected,
            msg.cpr_decoded,
            false,
            msg.addr,
            msg.signal_level,
            now,
        );

        // update current aircraft snapshot
        let snapshot = snapshot_from_message(msg);
        self.current_aircraft_states.insert(msg.addr, snapshot);

        // high/max per-message output
        match self.level {
            ConsoleLevel::High => {
                if let Some(line) = format_high(msg) {
                    output.push(line);
                }
            }
            ConsoleLevel::Max => {
                if let Some(line) = format_max(msg) {
                    output.push(line);
                }
            }
            _ => {}
        }

        output
    }

    pub fn flush_low(&mut self, now: Instant, currently_tracked: u32) -> Vec<String> {
        let mut output = Vec::new();
        if now.duration_since(self.last_low_report).as_secs() >= 60 {
            let stats = self.stats.aggregate(now, currently_tracked, self.start_time);
            let line = format_low(&stats);
            output.push(line);
            self.last_low_report = now;
        }
        output
    }

    pub fn flush_medium(&mut self, now: Instant) -> Vec<String> {
        let mut output = Vec::new();
        if now.duration_since(self.last_medium_report).as_secs() >= self.medium_interval_secs {
            for (&icao, current) in &self.current_aircraft_states {
                let empty_snapshot = AircraftSnapshot { icao, baro_alt: None, geom_alt: None, gs: None, track: None, baro_rate: None, callsign: None, lat: None, lon: None, signal_level: None, squawk: None, category: None, airground: None };
                let changes = if let Some(prev) = self.last_aircraft_states.get(&icao) {
                    diff_snapshots(prev, current)
                } else {
                    diff_snapshots(&empty_snapshot, current)
                };
                if !changes.is_empty() {
                    output.push(format_medium(icao, &changes));
                }
            }
            // swap: current states become the new baseline
            std::mem::swap(&mut self.last_aircraft_states, &mut self.current_aircraft_states);
            self.current_aircraft_states.clear();
            self.last_medium_report = now;
        }
        output
    }

    pub fn flush_low_final(&mut self, currently_tracked: u32) -> Vec<String> {
        let now = Instant::now();
        self.last_low_report = now.checked_sub(std::time::Duration::from_secs(120)).unwrap_or(now); // force flush
        self.flush_low(now, currently_tracked)
    }

    pub fn record_drop(&mut self) {
        self.stats.record_drop();
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default, clippy::len_zero)]
mod tests {
    use super::*;
    use crate::console::state::ConsoleLevel;
    use crate::types::ModesMessage;

    #[test]
    fn test_outputter_low_tier() {
        let mut out = Outputter::new(ConsoleLevel::Low, 10);
        let now = Instant::now();

        for i in 0..100 {
            let mut msg = ModesMessage::default();
            msg.addr = 0xA43EA2 + (i % 10);
            msg.msgtype = 17;
            msg.metype = 19;
            msg.crc_ok = true;
            msg.signal_level = -12.1;
            let t = now + std::time::Duration::from_secs(i as u64);
            out.feed(&msg, t);
        }

        let lines = out.flush_low(now + std::time::Duration::from_secs(60), 5);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("msgs/s="));
    }

    #[test]
    fn test_outputter_high_tier() {
        let mut out = Outputter::new(ConsoleLevel::High, 10);
        let now = Instant::now();

        let mut msg = ModesMessage::default();
        msg.addr = 0xA43EA2;
        msg.msgtype = 17;
        msg.metype = 19;
        msg.mesub = 1;
        msg.baro_alt_valid = true;
        msg.baro_alt = 27675;
        msg.gs_valid = true;
        msg.gs = 378.5;
        msg.crc_ok = true;
        msg.signal_level = -12.1;

        let lines = out.feed(&msg, now);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("A43EA2"));
        assert!(lines[0].contains("DF17"));
        assert!(lines[0].contains("vel"));
    }

    #[test]
    fn test_outputter_max_tier() {
        let mut out = Outputter::new(ConsoleLevel::Max, 10);
        let now = Instant::now();

        let mut msg = ModesMessage::default();
        msg.addr = 0xA6C311;
        msg.msgtype = 11;
        msg.crc_ok = true;
        msg.signal_level = -24.2;

        let lines = out.feed(&msg, now);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("DF11"));
        assert!(lines[0].contains("all-call"));
    }

    #[test]
    fn test_outputter_medium_change_detection() {
        let mut out = Outputter::new(ConsoleLevel::Medium, 1);
        let now = Instant::now();

        // Send first message with alt 10000
        let mut msg1 = ModesMessage::default();
        msg1.addr = 0xA43EA2;
        msg1.msgtype = 17;
        msg1.baro_alt_valid = true;
        msg1.baro_alt = 10000;
        msg1.crc_ok = true;
        msg1.signal_level = -12.1;
        out.feed(&msg1, now);

        // flush first to establish baseline
        let first = out.flush_medium(now + std::time::Duration::from_secs(2));
        assert_eq!(first.len(), 1);

        // Send second message with alt 10500 (change!)
        let mut msg2 = ModesMessage::default();
        msg2.addr = 0xA43EA2;
        msg2.msgtype = 17;
        msg2.baro_alt_valid = true;
        msg2.baro_alt = 10500;
        msg2.crc_ok = true;
        msg2.signal_level = -12.1;
        out.feed(&msg2, now + std::time::Duration::from_secs(3));

        // Should show alt change
        let lines = out.flush_medium(now + std::time::Duration::from_secs(5));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("▲"), "expected ▲ in '{}'", lines[0]);
    }

    #[test]
    fn test_outputter_medium_no_change() {
        let mut out = Outputter::new(ConsoleLevel::Medium, 1);
        let now = Instant::now();

        let mut msg = ModesMessage::default();
        msg.addr = 0xA43EA2;
        msg.msgtype = 17;
        msg.crc_ok = true;
        msg.signal_level = -12.1;
        out.feed(&msg, now);

        // first flush establishes baseline with signal_level change
        let first = out.flush_medium(now + std::time::Duration::from_secs(2));
        assert!(first.len() >= 1, "first flush should report new aircraft");

        // feed same message again (no new data)
        let mut msg2 = ModesMessage::default();
        msg2.addr = 0xA43EA2;
        msg2.msgtype = 17;
        msg2.crc_ok = true;
        msg2.signal_level = -12.1;
        out.feed(&msg2, now + std::time::Duration::from_secs(3));

        // nothing changed from baseline
        let lines = out.flush_medium(now + std::time::Duration::from_secs(5));
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_outputter_low_tier_not_yet() {
        let mut out = Outputter::new(ConsoleLevel::Low, 10);
        let now = Instant::now();

        let lines = out.flush_low(now + std::time::Duration::from_secs(30), 0);
        assert!(lines.is_empty(), "should not flush before 60s");
    }
}
