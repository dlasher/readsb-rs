use async_trait::async_trait;
use std::io;
use super::traits::SdrDevice;

#[repr(packed)]
pub struct DongleInfo {
    pub magic: [u8; 4],
    pub tuner_type: u32,
    pub tuner_gain_count: u32,
}

#[repr(packed)]
pub struct RtltcpCommand {
    pub cmd: u8,
    pub param: u32,
}

pub const RTLTCP_SET_FREQ: u8 = 0x01;
pub const RTLTCP_SET_SAMPLE_RATE: u8 = 0x02;
pub const RTLTCP_SET_GAIN_MODE: u8 = 0x03;
pub const RTLTCP_SET_GAIN: u8 = 0x04;
pub const RTLTCP_SET_FREQ_CORR: u8 = 0x05;
pub const RTLTCP_SET_IF_GAIN: u8 = 0x06;
pub const RTLTCP_SET_DIRECT_SAMP: u8 = 0x09;
pub const RTLTCP_SET_OFFSET_TUNING: u8 = 0x0A;
pub const RTLTCP_SET_BIAS_TEE: u8 = 0x0E;

pub struct RtlSdrDevice {
    device_index: u32,
    freq_hz: u32,
    gain_db: f32,
    sample_rate: u32,
    host: Option<String>,
    port: Option<u16>,
    direct_samp: Option<u8>,
    offset_tune: bool,
    bias_tee: bool,
}

impl RtlSdrDevice {
    pub fn new(device_index: u32) -> Self {
        RtlSdrDevice {
            device_index,
            freq_hz: 1090000000,
            gain_db: 49.6,
            sample_rate: 2400000,
            host: None,
            port: None,
            direct_samp: None,
            offset_tune: false,
            bias_tee: false,
        }
    }

    pub fn with_rtl_tcp(host: String, port: u16) -> Self {
        RtlSdrDevice {
            device_index: 0,
            freq_hz: 1090000000,
            gain_db: 49.6,
            sample_rate: 2400000,
            host: Some(host),
            port: Some(port),
            direct_samp: None,
            offset_tune: false,
            bias_tee: false,
        }
    }

    pub fn set_direct_samp(&mut self, mode: u8) {
        self.direct_samp = Some(mode);
    }

    pub fn set_offset_tune(&mut self, enable: bool) {
        self.offset_tune = enable;
    }

    pub fn set_bias_tee(&mut self, enable: bool) {
        self.bias_tee = enable;
    }
}

#[async_trait]
impl SdrDevice for RtlSdrDevice {
    async fn open(&mut self) -> io::Result<()> {
        unimplemented!("requires rtlsdr-sys FFI bindings")
    }

    async fn close(&mut self) -> io::Result<()> {
        unimplemented!()
    }

    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> {
        self.freq_hz = freq_hz;
        Ok(())
    }

    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        self.gain_db = gain_db;
        Ok(())
    }

    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> {
        self.sample_rate = rate_hz;
        Ok(())
    }

    async fn read_samples(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        unimplemented!()
    }

    fn name(&self) -> &str {
        if self.host.is_some() {
            "rtl_tcp"
        } else {
            "rtlsdr"
        }
    }
}

pub struct GainStats {
    pub loud_events: u64,
    pub noise_low_samples: u64,
    pub noise_high_samples: u64,
    pub total_samples: u64,
}

pub struct AgcState {
    pub slow_rise: i32,
    pub next_raise_agc: i64,
    pub loud_rebound: f32,
}
