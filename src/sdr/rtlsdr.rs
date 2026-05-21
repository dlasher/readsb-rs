use async_trait::async_trait;
use std::io;
use super::traits::SdrDevice;

/// Safe Send wrapper for rtl_sdr_rs::RtlSdr.
/// The inner type is not automatically Send because the Tuner trait
/// doesn't require Send, but all implementations (NoTuner, R82xx)
/// contain only Send-safe integer fields. Device is backed by
/// rusb::DeviceHandle which is documented as Send.
struct SdrHandle(rtl_sdr_rs::RtlSdr);
unsafe impl Send for SdrHandle {}
unsafe impl Sync for SdrHandle {}

pub struct RtlSdrDevice {
    device_index: u32,
    freq_hz: u32,
    gain_db: f32,
    sample_rate: u32,
    handle: Option<SdrHandle>,
}

impl SdrHandle {
    fn inner(&self) -> &rtl_sdr_rs::RtlSdr {
        &self.0
    }

    fn inner_mut(&mut self) -> &mut rtl_sdr_rs::RtlSdr {
        &mut self.0
    }

    fn open(device_index: u32, freq_hz: u32, sample_rate: u32, gain_db: f32) -> io::Result<Self> {
        let mut handle = rtl_sdr_rs::RtlSdr::open_with_index(device_index as usize)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        handle.set_sample_rate(sample_rate)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        handle.set_center_freq(freq_hz)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let gain_tenths = (gain_db * 10.0) as i32;
        handle.set_tuner_gain(rtl_sdr_rs::TunerGain::Manual(gain_tenths))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        handle.reset_buffer()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(SdrHandle(handle))
    }
}

impl RtlSdrDevice {
    pub fn new(device_index: u32) -> Self {
        RtlSdrDevice {
            device_index,
            freq_hz: 1090000000,
            gain_db: 49.6,
            sample_rate: 2400000,
            handle: None,
        }
    }
}

#[async_trait]
impl SdrDevice for RtlSdrDevice {
    async fn open(&mut self) -> io::Result<()> {
        let handle = SdrHandle::open(self.device_index, self.freq_hz, self.sample_rate, self.gain_db)?;
        self.handle = Some(handle);
        Ok(())
    }

    async fn close(&mut self) -> io::Result<()> {
        self.handle.take();
        Ok(())
    }

    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> {
        self.freq_hz = freq_hz;
        if let Some(ref mut handle) = self.handle {
            handle.inner_mut().set_center_freq(freq_hz)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        self.gain_db = gain_db;
        if let Some(ref mut handle) = self.handle {
            let gain_tenths = (gain_db * 10.0) as i32;
            handle.inner_mut().set_tuner_gain(rtl_sdr_rs::TunerGain::Manual(gain_tenths))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> {
        self.sample_rate = rate_hz;
        if let Some(ref mut handle) = self.handle {
            handle.inner_mut().set_sample_rate(rate_hz)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(())
    }

    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let handle = self.handle.as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "device not opened"))?;
        handle.inner().read_sync(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
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
