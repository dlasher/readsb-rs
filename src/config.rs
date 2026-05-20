use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "readsb", version, about = "Mode-S/ADSB/TIS message decoder")]
pub struct ReadsbConfig {
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

    #[arg(long)]
    pub rtltcp_direct_samp: Option<u8>,
    #[arg(long)]
    pub rtltcp_offset_tune: Option<bool>,
    #[arg(long)]
    pub rtltcp_bias_tee: Option<bool>,

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

    #[arg(long)]
    pub json_dir: Option<String>,
    #[arg(long)]
    pub json_globe_index: bool,
    #[arg(long)]
    pub json_reliable: Option<i32>,
    #[arg(long)]
    pub json_trace_interval: Option<i64>,

    #[arg(long)]
    pub lat: Option<f64>,
    #[arg(long)]
    pub lon: Option<f64>,
    #[arg(long)]
    pub max_range: Option<f64>,

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

    #[arg(long, default_value_t = 2)]
    pub decode_threads: u32,
    #[arg(long)]
    pub aggressive: bool,

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
