use std::io;
use super::traits::SdrDevice;
use super::ifile::IFileDevice;
use super::rtlsdr::{MockSdrDevice, RtlSdrDevice};
use super::rtl_tcp::RtlTcpClient;

#[derive(Clone)]
pub enum SdrType {
    IFile(String),
    RtlSdr(u32),
    RtlTcp(String, u16),
    Mock(Vec<u8>),
}

pub struct SdrManager {
    device: Option<Box<dyn SdrDevice>>,
}

impl SdrManager {
    pub fn new() -> Self {
        SdrManager { device: None }
    }

    pub fn create_device(sdr_type: SdrType) -> Box<dyn SdrDevice> {
        match sdr_type {
            SdrType::IFile(path) => Box::new(IFileDevice::new(path)),
            SdrType::RtlSdr(idx) => Box::new(RtlSdrDevice::new(idx)),
            SdrType::RtlTcp(host, port) => Box::new(RtlTcpClient::new(host, port)),
            SdrType::Mock(data) => Box::new(MockSdrDevice::new(data)),
        }
    }

    /// Create an RtlTcpClient with the ring buffer pre-configured.
    pub fn create_rtl_tcp_device(
        host: String,
        port: u16,
        ringbuf_capacity: usize,
        ringbuf_chunk_size: usize,
    ) -> Box<dyn SdrDevice> {
        let mut client = RtlTcpClient::new(host, port);
        client.set_ringbuf(ringbuf_capacity, ringbuf_chunk_size);
        Box::new(client)
    }

    async fn open_device(&mut self, device: Box<dyn SdrDevice>) -> io::Result<()> {
        self.device = Some(device);
        self.device.as_mut().unwrap().open().await
    }

    pub async fn open(&mut self, sdr_type: SdrType) -> io::Result<()> {
        self.device = Some(Self::create_device(sdr_type));
        self.device.as_mut().unwrap().open().await
    }

    /// Open a pre-created device (e.g., RtlTcpClient with ring buffer).
    pub async fn open_with_device(&mut self, device: Box<dyn SdrDevice>) -> io::Result<()> {
        self.open_device(device).await
    }

    pub async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(ref mut device) = self.device {
            device.read_samples(buf).await
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "No SDR device open"))
        }
    }

    pub async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        match &mut self.device {
            Some(device) => device.set_gain(gain_db).await,
            None => Err(io::Error::new(io::ErrorKind::NotConnected, "No SDR device open")),
        }
    }
}

impl Default for SdrManager {
    fn default() -> Self {
        Self::new()
    }
}
