use async_trait::async_trait;
use std::io;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;
use super::traits::SdrDevice;

/// RTL-TCP protocol commands
pub const RTLTCP_SET_FREQ: u8 = 0x01;
pub const RTLTCP_SET_SAMPLE_RATE: u8 = 0x02;
pub const RTLTCP_SET_GAIN_MODE: u8 = 0x03;
pub const RTLTCP_SET_GAIN: u8 = 0x04;
pub const RTLTCP_SET_FREQ_CORR: u8 = 0x05;
pub const RTLTCP_SET_IF_GAIN: u8 = 0x06;
pub const RTLTCP_SET_DIRECT_SAMP: u8 = 0x09;
pub const RTLTCP_SET_OFFSET_TUNING: u8 = 0x0A;
pub const RTLTCP_SET_BIAS_TEE: u8 = 0x0E;

/// RTL-TCP response status codes
const RTLTCP_CMD_SUCCESS: u8 = 0x01;
const RTLTCP_CMD_ERROR: u8 = 0xFF;

/// RTL-TCP client for remote RTL-SDR devices
pub struct RtlTcpClient {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    freq_hz: u32,
    sample_rate: u32,
    gain_db: f32,
    connected: bool,
}

impl RtlTcpClient {
    pub fn new(host: String, port: u16) -> Self {
        RtlTcpClient {
            host,
            port,
            stream: None,
            freq_hz: 1090000000,
            sample_rate: 2400000,
            gain_db: 49.6,
            connected: false,
        }
    }

    /// Write a command and read response
    async fn write_command(&mut self, cmd: u8, param: u32) -> io::Result<u8> {
        if let Some(ref mut stream) = self.stream {
            // Write command byte
            stream.write_all(&[cmd]).await?;
            // Write 4-byte parameter (big-endian)
            stream.write_all(&param.to_be_bytes()).await?;
            // Read 4-byte response
            let mut response = [0u8; 4];
            stream.read_exact(&mut response).await?;
            Ok(response[0])
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "RTL-TCP not connected"))
        }
    }

    /// Update sample rate
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate;
    }

    /// Update frequency
    pub fn set_freq(&mut self, freq: u32) {
        self.freq_hz = freq;
    }

    /// Update gain
    pub fn set_gain(&mut self, gain: f32) {
        self.gain_db = gain;
    }

    /// Read samples from the stream
    pub async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.stream {
            Some(stream) => {
                stream.read(buf).await
            }
            None => {
                Err(io::Error::new(io::ErrorKind::NotConnected, "RTL-TCP connection not open"))
            }
        }
    }
}

#[async_trait]
impl SdrDevice for RtlTcpClient {
    async fn open(&mut self) -> io::Result<()> {
        let max_retries = 3;
        let retry_delay = Duration::from_secs(5);

        for _attempt in 0..max_retries {
            match TcpStream::connect((self.host.as_str(), self.port)).await {
                Ok(mut stream) => {
                    // Send handshake: 0x00 byte
                    stream.write_all(&[0x00]).await?;
                    
                    // Read 4-byte response
                    let mut response = [0u8; 4];
                    stream.read_exact(&mut response).await?;
                    
                    // Check handshake success (response[0] should be 0x00)
                    if response[0] == 0x00 {
                        self.stream = Some(stream);
                        self.connected = true;
                        // Negotiate sample rate after connection
                        match self.write_command(RTLTCP_SET_SAMPLE_RATE, self.sample_rate).await {
                            Ok(RTLTCP_CMD_SUCCESS) => {},
                            Ok(_) => {},
                            Err(e) => {
                                self.connected = false;
                                return Err(e);
                            }
                        }
                        match self.write_command(RTLTCP_SET_FREQ, self.freq_hz).await {
                            Ok(RTLTCP_CMD_SUCCESS) => {},
                            Ok(_) => {},
                            Err(e) => {
                                self.connected = false;
                                return Err(e);
                            }
                        }
                        // Convert gain to tenths of dB (e.g., 49.6 -> 496)
                        let gain_tenths = (self.gain_db * 10.0) as i32;
                        match self.write_command(RTLTCP_SET_GAIN, gain_tenths as u32).await {
                            Ok(RTLTCP_CMD_SUCCESS) => {},
                            Ok(_) => {},
                            Err(e) => {
                                self.connected = false;
                                return Err(e);
                            }
                        }
                        return Ok(());
                    }
                    
                    // Handshake failed, close and retry
                    drop(stream);
                    tokio::time::sleep(retry_delay).await;
                }
                Err(_) => {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
        
        Err(io::Error::new(io::ErrorKind::ConnectionRefused, 
            "Failed to connect to RTL-TCP server"))
    }

    async fn close(&mut self) -> io::Result<()> {
        self.stream = None;
        self.connected = false;
        Ok(())
    }

    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> {
        self.set_freq(freq_hz);
        match self.write_command(RTLTCP_SET_FREQ, freq_hz).await {
            Ok(RTLTCP_CMD_SUCCESS) => Ok(()),
            Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "RTL-TCP command failed")),
            Err(e) => Err(e),
        }
    }

    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        self.set_gain(gain_db);
        let gain_tenths = (gain_db * 10.0) as i32;
        match self.write_command(RTLTCP_SET_GAIN, gain_tenths as u32).await {
            Ok(RTLTCP_CMD_SUCCESS) => Ok(()),
            Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "RTL-TCP command failed")),
            Err(e) => Err(e),
        }
    }

    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> {
        self.set_sample_rate(rate_hz);
        match self.write_command(RTLTCP_SET_SAMPLE_RATE, rate_hz).await {
            Ok(RTLTCP_CMD_SUCCESS) => Ok(()),
            Ok(_) => Err(io::Error::new(io::ErrorKind::Other, "RTL-TCP command failed")),
            Err(e) => Err(e),
        }
    }

    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_samples(buf).await
    }

    fn name(&self) -> &str {
        "rtl_tcp"
    }
}
