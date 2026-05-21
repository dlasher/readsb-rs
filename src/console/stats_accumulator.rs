use std::collections::{HashSet, VecDeque};
use std::time::Instant;

pub struct AggregatedStats {
    pub msgs_per_sec: f64,
    pub currently_tracked: u32,
    pub unique_aircraft_hour: u32,
    pub crc_fail_pct: f64,
    pub bitfix_pct: f64,
    pub sig_avg: f64,
    pub sig_max: f64,
    pub sig_min: f64,
    pub drops: u64,
    pub df_distribution: Vec<(u8, u32)>,
    pub cpr_ok_pct: f64,
    pub uptime_secs: u64,
}

pub struct StatsAccumulator {
    message_timestamps: VecDeque<Instant>,
    df_types: VecDeque<(Instant, u8)>,
    crc_fail_count: u32,
    crc_total_count: u32,
    bitfix_count: u32,
    cpr_global_ok: u32,
    cpr_global_bad: u32,
    cpr_local_ok: u32,
    signal_readings: VecDeque<(Instant, f64)>,
    drop_count: u64,
    unique_aircraft_last_hour: HashSet<u32>,
}

impl StatsAccumulator {
    pub fn new() -> Self {
        StatsAccumulator {
            message_timestamps: VecDeque::new(),
            df_types: VecDeque::new(),
            crc_fail_count: 0,
            crc_total_count: 0,
            bitfix_count: 0,
            cpr_global_ok: 0,
            cpr_global_bad: 0,
            cpr_local_ok: 0,
            signal_readings: VecDeque::new(),
            drop_count: 0,
            unique_aircraft_last_hour: HashSet::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_message(&mut self, df: u8, crc_ok: bool, crc_corrected: bool, cpr_ok: bool, cpr_local: bool, icao: u32, signal: f64, now: Instant) {
        self.message_timestamps.push_back(now);
        self.df_types.push_back((now, df));
        self.crc_total_count += 1;
        if !crc_ok {
            self.crc_fail_count += 1;
        } else if crc_corrected {
            self.bitfix_count += 1;
        }
        if cpr_ok {
            if cpr_local {
                self.cpr_local_ok += 1;
            } else {
                self.cpr_global_ok += 1;
            }
        } else {
            self.cpr_global_bad += 1;
        }
        self.unique_aircraft_last_hour.insert(icao);
        if signal != 0.0 {
            self.signal_readings.push_back((now, signal));
        }
    }

    pub fn record_drop(&mut self) {
        self.drop_count += 1;
    }

    pub fn aggregate(&mut self, now: Instant, currently_tracked: u32, start_time: Instant) -> AggregatedStats {
        let window_start = now - std::time::Duration::from_secs(60);
        while self.message_timestamps.front().map(|&t| t < window_start).unwrap_or(false) {
            self.message_timestamps.pop_front();
        }
        while self.signal_readings.front().map(|&(t, _)| t < window_start).unwrap_or(false) {
            self.signal_readings.pop_front();
        }
        while self.df_types.front().map(|&(t, _)| t < window_start).unwrap_or(false) {
            self.df_types.pop_front();
        }

        let window_msgs = self.message_timestamps.len();
        let msgs_per_sec = window_msgs as f64 / 60.0;

        let total_ok = self.crc_total_count.max(1);
        let crc_fail_pct = (self.crc_fail_count as f64 / total_ok as f64) * 100.0;
        let bitfix_pct = (self.bitfix_count as f64 / total_ok as f64) * 100.0;

        let cpr_total = (self.cpr_global_ok + self.cpr_global_bad + self.cpr_local_ok).max(1);
        let cpr_ok_total = self.cpr_global_ok + self.cpr_local_ok;
        let cpr_ok_pct = (cpr_ok_total as f64 / cpr_total as f64) * 100.0;

        let (sig_avg, sig_max, sig_min) = if self.signal_readings.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let sum: f64 = self.signal_readings.iter().map(|&(_, s)| s).sum();
            let max = self.signal_readings.iter().map(|&(_, s)| s).fold(f64::NEG_INFINITY, f64::max);
            let min = self.signal_readings.iter().map(|&(_, s)| s).fold(f64::INFINITY, f64::min);
            (sum / self.signal_readings.len() as f64, max, min)
        };

        let mut df_counts = [0u32; 32];
        for &(_, df) in &self.df_types {
            if (df as usize) < 32 {
                df_counts[df as usize] += 1;
            }
        }
        let mut df_dist: Vec<(u8, u32)> = df_counts.iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .map(|(i, &c)| (i as u8, c))
            .collect();
        df_dist.sort_by_key(|b| std::cmp::Reverse(b.1));

        let uah = self.unique_aircraft_last_hour.len() as u32;


        AggregatedStats {
            msgs_per_sec,
            currently_tracked,
            unique_aircraft_hour: uah,
            crc_fail_pct,
            bitfix_pct,
            sig_avg,
            sig_max,
            sig_min,
            drops: self.drop_count,
            df_distribution: df_dist,
            cpr_ok_pct,
            uptime_secs: now.duration_since(start_time).as_secs(),
        }
    }

    pub fn reset(&mut self) {
        self.message_timestamps.clear();
        self.df_types.clear();
        self.crc_fail_count = 0;
        self.crc_total_count = 0;
        self.bitfix_count = 0;
        self.cpr_global_ok = 0;
        self.cpr_global_bad = 0;
        self.cpr_local_ok = 0;
        self.signal_readings.clear();
        self.drop_count = 0;
        self.unique_aircraft_last_hour.clear();
    }
}

impl Default for StatsAccumulator {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_stats_accumulator_basic() {
        let mut acc = StatsAccumulator::new();
        let now = Instant::now();
        let start = now;

        acc.record_message(17, true, false, true, false, 0xA43EA2, -12.1, now);
        acc.record_message(11, true, false, false, false, 0xA6C311, -24.2, now);
        acc.record_message(17, true, true, true, true, 0xA43EA2, -12.5, now + Duration::from_secs(1));

        let stats = acc.aggregate(now + Duration::from_secs(60), 2, start);

        assert!(stats.msgs_per_sec > 0.0);
        assert!(stats.crc_fail_pct < 100.0);
        assert!(stats.bitfix_pct > 0.0);
        assert_eq!(stats.currently_tracked, 2);
        assert_eq!(stats.unique_aircraft_hour, 2);
    }

    #[test]
    fn test_stats_accumulator_zero() {
        let mut acc = StatsAccumulator::new();
        let now = Instant::now();
        let stats = acc.aggregate(now + Duration::from_secs(60), 0, now);
        assert_eq!(stats.msgs_per_sec, 0.0);
        assert_eq!(stats.currently_tracked, 0);
        assert_eq!(stats.sig_avg, 0.0);
    }

    #[test]
    fn test_stats_accumulator_with_drops() {
        let mut acc = StatsAccumulator::new();
        let now = Instant::now();
        acc.record_drop();
        acc.record_drop();
        acc.record_drop();
        let stats = acc.aggregate(now + Duration::from_secs(60), 1, now);
        assert_eq!(stats.drops, 3);
    }

    #[test]
    fn test_reset() {
        let mut acc = StatsAccumulator::new();
        let now = Instant::now();
        acc.record_message(17, true, false, true, false, 0xA43EA2, -12.1, now);
        acc.reset();
        let stats = acc.aggregate(now + Duration::from_secs(60), 0, now);
        assert_eq!(stats.msgs_per_sec, 0.0);
        assert_eq!(stats.unique_aircraft_hour, 0);
    }

    #[test]
    fn test_stats_accumulator_df_windowed() {
        let mut acc = StatsAccumulator::new();
        let start = Instant::now();

        // 100 DF17 messages at t=0
        for _ in 0..100 {
            acc.record_message(17, true, false, false, false, 0xA43EA2, -12.1, start);
        }

        // 2 DF11 messages at t=90s (outside the 60s window from t=60 to t=120)
        let t90 = start + Duration::from_secs(90);
        acc.record_message(11, true, false, false, false, 0xA6C311, -12.1, t90);
        acc.record_message(11, true, false, false, false, 0xA6C312, -12.1, t90);

        // Aggregate at t=120s — window = 60s spanning t=60 to t=120
        let stats = acc.aggregate(start + Duration::from_secs(120), 0, start);

        // DF17 should be windowed out (all at t=0, outside window)
        assert!(stats.df_distribution.iter().all(|(df, _)| *df != 17),
            "DF17 should be windowed out");
        // DF11 should have 2 entries (at t=90, inside window)
        assert!(stats.df_distribution.iter().any(|(df, count)| *df == 11 && *count == 2),
            "DF11 should have count 2 in window");
    }
}
