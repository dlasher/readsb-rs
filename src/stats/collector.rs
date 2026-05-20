pub struct Stats {
    pub demod_preambles: u32, pub demod_rejected_bad: u32, pub demod_rejected_unknown: u32,
    pub demod_accepted: [u32; 3],
    pub samples_processed: u64,
    pub cpr_global_ok: u32, pub cpr_global_bad: u32, pub cpr_local_ok: u32,
    pub remote_received_modeac: u32, pub remote_received_modes: u32,
    pub messages_total: u32, pub unique_aircraft: u32,
    pub network_bytes_in: u64, pub network_bytes_out: u64,
    pub range_histogram: [u32; 128],
    pub distance_max: f64, pub distance_min: f64,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            demod_preambles: 0, demod_rejected_bad: 0, demod_rejected_unknown: 0,
            demod_accepted: [0; 3], samples_processed: 0,
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
