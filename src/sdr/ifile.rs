use async_trait::async_trait;
use std::io;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use super::traits::SdrDevice;

pub struct IFileDevice {
    path: String,
    file: Option<File>,
}

impl IFileDevice {
    pub fn new(path: String) -> Self {
        IFileDevice { path, file: None }
    }
}

#[async_trait]
impl SdrDevice for IFileDevice {
    async fn open(&mut self) -> io::Result<()> {
        self.file = Some(File::open(&self.path).await?);
        Ok(())
    }

    async fn close(&mut self) -> io::Result<()> {
        self.file = None;
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
        if let Some(ref mut file) = self.file {
            file.read(buf).await
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "File not open"))
        }
    }

    fn name(&self) -> &str {
        "ifile"
    }
}
