use async_trait::async_trait;
use std::io;
use super::traits::SdrDevice;

pub struct RtlSdrDevice {
    device_index: u32,
    freq_hz: u32,
    gain_db: f32,
    sample_rate: u32,
    handle: Option<rtl_sdr_rs::RtlSdr>,
}
// rtl_sdr_rs::RtlSdr is not Send because Box<dyn Tuner> lacks Send bounds,
// but all implementations (NoTuner, R82xx) contain only integer fields.
// Device is backed by rusb::DeviceHandle<Context> which is Send.
// &mut self ensures exclusive access — no concurrent mutations.
unsafe impl Send for RtlSdrDevice {}
unsafe impl Sync for RtlSdrDevice {}

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
        let mut h = rtl_sdr_rs::RtlSdr::open_with_index(self.device_index as usize)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        h.set_sample_rate(self.sample_rate)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        h.set_center_freq(self.freq_hz)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        let t = (self.gain_db * 10.0) as i32;
        h.set_tuner_gain(rtl_sdr_rs::TunerGain::Manual(t))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        h.reset_buffer()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        self.handle = Some(h);
        Ok(())
    }

    async fn close(&mut self) -> io::Result<()> {
        self.handle.take();
        Ok(())
    }

    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> {
        self.freq_hz = freq_hz;
        if let Some(ref mut h) = self.handle {
            let _ = h.set_center_freq(freq_hz);
        }
        Ok(())
    }

    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        self.gain_db = gain_db;
        if let Some(ref mut h) = self.handle {
            let t = (gain_db * 10.0) as i32;
            let _ = h.set_tuner_gain(rtl_sdr_rs::TunerGain::Manual(t));
        }
        Ok(())
    }

    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> {
        self.sample_rate = rate_hz;
        if let Some(ref mut h) = self.handle {
            let _ = h.set_sample_rate(rate_hz);
        }
        Ok(())
    }

    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let h = self.handle.as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "device not opened"))?;
        h.read_sync(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
    }

    fn name(&self) -> &str {
        "rtlsdr"
    }
}

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
    async fn set_freq(&mut self, _f: u32) -> io::Result<()> { Ok(()) }
    async fn set_gain(&mut self, _g: f32) -> io::Result<()> { Ok(()) }
    async fn set_sample_rate(&mut self, _r: u32) -> io::Result<()> { Ok(()) }
    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let remaining = self.data.len() - self.pos;
        let to_read = buf.len().min(remaining);
        buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
    fn name(&self) -> &str { "mock" }
}

pub struct GainStats { pub loud_events: u64, pub noise_low_samples: u64, pub noise_high_samples: u64, pub total_samples: u64 }
pub struct AgcState { pub slow_rise: i32, pub next_raise_agc: i64, pub loud_rebound: f32 }
