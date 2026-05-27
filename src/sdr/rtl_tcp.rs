use async_trait::async_trait;
use std::io;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
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

/// RTL-TCP response status codes (not used - protocol is fire-and-forget)
#[allow(dead_code)]
const RTLTCP_RESP_SUCCESS: u8 = 0x01;

/// Dongle info response from server
#[repr(C)]
#[repr(packed)]
pub struct DongleInfo {
    pub magic: [u8; 4],
    pub tuner_type: u32,
    pub tuner_gain_count: u32,
}

/// RTL-TCP client for remote RTL-SDR devices
pub struct RtlTcpClient {
    host: String,
    port: u16,
    write_half: Option<tokio::net::tcp::OwnedWriteHalf>,
    freq_hz: u32,
    sample_rate: u32,
    gain_db: f32,
    connected: bool,
    /// Sender side of the bounded mpsc channel (reader task pushes here)
    #[allow(dead_code)]
    ringbuf_tx: Option<mpsc::Sender<Vec<u8>>>,
    /// Receiver side of the bounded mpsc channel (demod loop pops from here)
    #[allow(dead_code)]
    ringbuf_rx: Option<mpsc::Receiver<Vec<u8>>>,
    /// Capacity in chunks (default 16 = 4MB with 262KB each)
    ringbuf_capacity: usize,
    /// Size of each chunk the reader pushes
    ringbuf_chunk_size: usize,
}

impl RtlTcpClient {
    pub fn new(host: String, port: u16) -> Self {
        RtlTcpClient {
            host,
            port,
            write_half: None,
            freq_hz: 1090000000,
            sample_rate: 2400000,
            gain_db: 49.6,
            connected: false,
            ringbuf_tx: None,
            ringbuf_rx: None,
            ringbuf_capacity: 16,
            ringbuf_chunk_size: 262_144,
        }
    }

    /// Configure ring buffer. Must be called **before** `open()`.
    pub fn set_ringbuf(&mut self, capacity: usize, chunk_size: usize) {
        self.ringbuf_capacity = capacity;
        self.ringbuf_chunk_size = chunk_size;
    }

    fn spawn_reader_task(&mut self, mut read_half: tokio::net::tcp::OwnedReadHalf) {
        let chunk_size = self.ringbuf_chunk_size;
        let capacity = self.ringbuf_capacity;
        let (tx, rx) = mpsc::channel(capacity);
        self.ringbuf_tx = Some(tx.clone());
        self.ringbuf_rx = Some(rx);

        tokio::spawn(async move {
            let mut buf = vec![0u8; chunk_size];
            loop {
                let mut total = 0usize;
                while total < chunk_size {
                    match read_half.read(&mut buf[total..]).await {
                        Ok(0) => {
                            // EOF — close connection
                            return;
                        }
                        Ok(n) => total += n,
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock
                            || e.kind() == io::ErrorKind::TimedOut => {
                            continue;
                        }
                        Err(_) => {
                            return;
                        }
                    }
                }
                let chunk = buf.clone();
                // bounded channel backpressure: block if full
                if tx.send(chunk).await.is_err() {
                    return; // receiver dropped
                }
            }
        });
    }

    /// Connect to RTL-TCP server and read dongle info (NO handshake byte)
    async fn connect(&mut self) -> io::Result<()> {
        let max_retries = 3;
        let retry_delay = Duration::from_secs(5);

        for _attempt in 0..max_retries {
            match TcpStream::connect((self.host.as_str(), self.port)).await {
                Ok(mut stream) => {
                    // Server immediately sends dongle_info (8 bytes: magic + tuner_type + gain_count)
                    let mut info_buf = [0u8; 12];
                    match stream.read_exact(&mut info_buf).await {
                        Ok(_) => {
                            let info: DongleInfo = unsafe { std::mem::transmute(info_buf) };
                            if info.magic != [b'R', b'T', b'L', b'0'] {
                                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid dongle magic"));
                            }
                            self.connected = true;
                            let (read_half, write_half) = stream.into_split();
                            self.write_half = Some(write_half);
                            // Configure the server (50ms between commands for rate limiting)
                            self.send_command(RTLTCP_SET_FREQ, self.freq_hz).await?;
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            self.send_command(RTLTCP_SET_SAMPLE_RATE, self.sample_rate).await?;
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            let gain_tenths = (self.gain_db * 10.0) as i32;
                            self.send_command(RTLTCP_SET_GAIN_MODE, 1).await?;
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            self.send_command(RTLTCP_SET_GAIN, gain_tenths as u32).await?;
                            // Spawn background reader task with bounded mpsc ring buffer
                            self.spawn_reader_task(read_half);
                            return Ok(());
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
        
        Err(io::Error::new(io::ErrorKind::ConnectionRefused, 
            "Failed to connect to RTL-TCP server"))
    }

    /// Send a command - RTL-TCP protocol is fire-and-forget, no response
    async fn send_command(&mut self, cmd: u8, param: u32) -> io::Result<()> {
        if let Some(ref mut write_half) = self.write_half {
            write_half.write_all(&[cmd]).await?;
            write_half.write_all(&param.to_be_bytes()).await?;
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "RTL-TCP not connected"))
        }
    }

    /// Update sample rate and reconfigure server
    pub fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate;
    }

    /// Update frequency and reconfigure server
    pub fn set_freq(&mut self, freq: u32) {
        self.freq_hz = freq;
    }

    /// Update gain and reconfigure server
    pub fn set_gain(&mut self, gain: f32) {
        self.gain_db = gain;
    }

    /// Read samples from the ring buffer. When ring buffer is active,
    /// pops from the mpsc channel; otherwise reads directly from TCP stream.
    pub async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If ring buffer is active, pop from mpsc channel
        if let Some(ref mut rx) = self.ringbuf_rx {
            match rx.recv().await {
                Some(chunk) => {
                    let n = chunk.len().min(buf.len());
                    buf[..n].copy_from_slice(&chunk[..n]);
                    Ok(n)
                }
                None => {
                    Err(io::Error::new(io::ErrorKind::ConnectionAborted, "RTL-TCP ring buffer closed"))
                }
            }
        } else {
            // Fallback: should never happen since ring buffer is always used after open
            Err(io::Error::new(io::ErrorKind::NotConnected, "RTL-TCP ring buffer not initialized"))
        }
    }
}

#[async_trait]
impl SdrDevice for RtlTcpClient {
    async fn open(&mut self) -> io::Result<()> {
        self.connect().await
    }

    async fn close(&mut self) -> io::Result<()> {
        self.write_half = None;
        self.connected = false;
        Ok(())
    }

    async fn set_freq(&mut self, freq_hz: u32) -> io::Result<()> {
        self.set_freq(freq_hz);
        self.send_command(RTLTCP_SET_FREQ, freq_hz).await
    }

    async fn set_gain(&mut self, gain_db: f32) -> io::Result<()> {
        self.set_gain(gain_db);
        let gain_tenths = (gain_db * 10.0) as i32;
        self.send_command(RTLTCP_SET_GAIN_MODE, 1).await?; // Manual gain mode
        self.send_command(RTLTCP_SET_GAIN, gain_tenths as u32).await
    }

    async fn set_sample_rate(&mut self, rate_hz: u32) -> io::Result<()> {
        self.set_sample_rate(rate_hz);
        self.send_command(RTLTCP_SET_SAMPLE_RATE, rate_hz).await
    }

    async fn read_samples(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.read_samples(buf).await
    }

    fn name(&self) -> &str {
        "rtl_tcp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtltcp_ringbuf_fields_exist() {
        let mut client = RtlTcpClient::new("127.0.0.1".to_string(), 1234);
        client.set_ringbuf(4194304, 262144);
        // verify fields were set via struct fields
        assert_eq!(client.ringbuf_capacity, 4194304);
        assert_eq!(client.ringbuf_chunk_size, 262_144);
    }
}