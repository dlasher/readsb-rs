use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "readsb", version, about = "Mode-S/ADSB/TIS message decoder")]
pub struct ReadsbConfig {
    #[arg(long, default_value = "rtlsdr", env = "READSB_DEVICE_TYPE")]
    pub device_type: String,
    #[arg(long, env = "READSB_DEVICE")]
    pub device: Option<String>,
    #[arg(long, env = "READSB_GAIN")]
    pub gain: Option<f32>,
    #[arg(long, default_value_t = 1090000000, env = "READSB_FREQ")]
    pub freq: u32,
    #[arg(long, env = "READSB_PPM")]
    pub ppm: Option<i32>,

    #[arg(long, env = "READSB_RTLTCP_DIRECT_SAMP")]
    #[allow(dead_code)]
    pub rtltcp_direct_samp: Option<u8>,
    #[arg(long, env = "READSB_RTLTCP_OFFSET_TUNE")]
    #[allow(dead_code)]
    pub rtltcp_offset_tune: Option<bool>,
    #[arg(long, env = "READSB_RTLTCP_BIAS_TEE")]
    #[allow(dead_code)]
    pub rtltcp_bias_tee: Option<bool>,

    #[arg(long, env = "READSB_NET")]
    pub net: bool,
    #[arg(long, default_value = "30005", env = "READSB_NET_BO_PORT")]
    pub net_bo_port: String,
    #[arg(long, default_value = "30002", env = "READSB_NET_RI_PORT")]
    pub net_ri_port: String,
    #[arg(long, default_value = "30003", env = "READSB_NET_SBS_PORT")]
    pub net_sbs_port: String,
    #[arg(long, env = "READSB_NET_BIND_ADDRESS")]
    pub net_bind_address: Option<String>,

    #[arg(long, env = "READSB_JSON_DIR")]
    pub json_dir: Option<String>,
    #[arg(long, env = "READSB_JSON_GLOBE_INDEX")]
    pub json_globe_index: bool,
    #[arg(long, env = "READSB_JSON_RELIABLE")]
    pub json_reliable: Option<i32>,
    #[arg(long, env = "READSB_JSON_TRACE_INTERVAL")]
    #[allow(dead_code)] // pending Phase 5
    pub json_trace_interval: Option<i64>,

    #[arg(long, env = "READSB_LAT")]
    pub lat: Option<f64>,
    #[arg(long, env = "READSB_LON")]
    pub lon: Option<f64>,
    #[arg(long, env = "READSB_MAX_RANGE")]
    #[allow(dead_code)] // pending range filtering feature
    pub max_range: Option<f64>,

    #[arg(long, env = "READSB_PREAMBLE_THRESHOLD")]
    pub preamble_threshold: Option<u32>,
    #[arg(long, env = "READSB_AGC")]
    pub agc: bool,

    #[arg(long, env = "READSB_DEBUG_NET")]
    pub debug_net: bool,
    #[arg(long, env = "READSB_DEBUG_CPR")]
    pub debug_cpr: bool,
    #[arg(long, env = "READSB_DEBUG_GARBAGE")]
    pub debug_garbage: bool,
    #[arg(long, env = "READSB_DEBUG_API")]
    pub debug_api: bool,
    #[arg(long, env = "READSB_QUIET")]
    pub quiet: bool,

    #[arg(long, default_value_t = 2, env = "READSB_DECODE_THREADS")]
    pub decode_threads: u32,
    #[arg(long, env = "READSB_AGGRESSIVE")]
    pub aggressive: bool,
    #[arg(long, default_value_t = true, env = "READSB_MULTI_PASS")]
    pub multi_pass: bool,
    #[arg(long, default_value_t = 0.8, env = "READSB_MULTI_PASS_MARGIN")]
    pub multi_pass_margin: f32,
    #[arg(long, default_value_t = 4194304, env = "READSB_RINGBUF_SIZE")]
    pub ringbuf_size: usize,

    #[arg(long, env = "READSB_IFILE")]
    pub ifile: Option<String>,
    #[arg(long, env = "READSB_IFORMAT")]
    pub iformat: Option<String>,
}

impl ReadsbConfig {
    pub fn from_cli() -> Self {
        Self::parse()
    }
}
