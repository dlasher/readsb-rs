use async_trait::async_trait;
use std::io;
use super::traits::SdrDevice;

#[allow(dead_code)]
pub struct RtlSdrDevice {
    device_index: u32,
    freq_hz: u32,
    gain_db: f32,
    sample_rate: u32,
}

impl RtlSdrDevice {
    pub fn new(device_index: u32) -> Self {
        RtlSdrDevice {
            device_index,
            freq_hz: 1090000000,
            gain_db: 49.6,
            sample_rate: 2400000,
        }
    }
}

#[async_trait]
impl SdrDevice for RtlSdrDevice {
    async fn open(&mut self) -> io::Result<()> {
        // USB mode - placeholder for FFI bindings
        Ok(())
    }

    async fn close(&mut self) -> io::Result<()> {
        Ok(())
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
        Err(io::Error::new(io::ErrorKind::Unsupported, "USB sample reading not implemented"))
    }

    fn name(&self) -> &str {
        "rtlsdr"
    }
}

/// Mock SDR device that returns canned sample data.
/// Used for testing without hardware.
#[allow(dead_code)]
pub struct MockSdrDevice {
    data: Vec<u8>,
    pos: usize,
}

impl MockSdrDevice {
    pub fn new(data: Vec<u8>) -> Self {
        MockSdrDevice { data, pos: 0 }
    }
}

#[async_trait]
impl SdrDevice for MockSdrDevice {
    async fn open(&mut self) -> io::Result<()> {
        self.pos = 0;
        Ok(())
    }

    async fn close(&mut self) -> io::Result<()> {
        self.pos = 0;
        Ok(())
    }

    async fn set_freq(&mut self, _freq_hz: u32) -> io::Result<()> {
        Ok(())
    }

    async fn set_gain(&mut self, _gain_db: f32) -> io::Result<()> {
        Ok(())
    }

    async fn set_sample_rate(&mut self, _rate_hz: u32) -> io::Result<()> {
        Ok(())
    }

    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self.data.len() - self.pos;
        let to_read = buf.len().min(remaining);
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }

    fn name(&self) -> &str {
        "mock"
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
