use async_trait::async_trait;
use std::io;

#[async_trait]
pub trait SdrDevice: Send + Sync {
    async fn open(&mut self) -> io::Result<()>;
    async fn close(&mut self) -> io::Result<()>;
    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()>;
    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()>;
    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()>;
    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn name(&self) -> &str;
}
