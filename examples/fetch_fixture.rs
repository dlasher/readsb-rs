use std::io::Write;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: fetch_fixture <host> <port> [duration_secs] [output_file]");
        std::process::exit(1);
    }
    let host = &args[1];
    let port: u16 = args[2].parse()?;
    let duration_secs: u64 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(3);
    let output_file = args.get(4).cloned().unwrap_or_else(|| "test_fixtures/rtltcp_sample.iq".to_string());

    eprintln!("Connecting to {}:{}...", host, port);
    let mut stream = TcpStream::connect((host.as_str(), port)).await?;

    // Read dongle info (12 bytes)
    let mut info = [0u8; 12];
    stream.read_exact(&mut info).await?;
    eprintln!("Dongle info: magic={:?}, tuner_type={}, gain_count={}",
        &info[0..4],
        u32::from_be_bytes([info[4], info[5], info[6], info[7]]),
        u32::from_be_bytes([info[8], info[9], info[10], info[11]]));

    // Configure: set sample rate to 2.4 MHz
    send_command(&mut stream, 0x02, 2400000).await?;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Set frequency to 1090 MHz (Mode-S)
    send_command(&mut stream, 0x01, 1090000000).await?;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Manual gain mode
    send_command(&mut stream, 0x03, 1).await?;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Set gain (496 = 49.6 dB * 10)
    send_command(&mut stream, 0x04, 496).await?;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Read samples
    let total_samples = duration_secs * 2400000 * 2; // 2 bytes per sample (SC16Q11)
    let mut samples = Vec::with_capacity(total_samples as usize);
    let mut buf = vec![0u8; 65536];
    let mut read: u64 = 0;

    eprintln!("Reading {} bytes of samples (~{}s)...", total_samples, duration_secs);
    while read < total_samples {
        let n = stream.read(&mut buf).await?;
        if n == 0 { break; }
        samples.extend_from_slice(&buf[..n]);
        read += n as u64;
    }

    // Write to file
    std::fs::create_dir_all("test_fixtures")?;
    let mut file = std::fs::File::create(output_file)?;
    file.write_all(&samples)?;

    eprintln!("Wrote {} bytes to test_fixtures/live_sample.iq", samples.len());
    Ok(())
}

async fn send_command(stream: &mut TcpStream, cmd: u8, param: u32) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    stream.write_all(&[cmd]).await?;
    stream.write_all(&param.to_be_bytes()).await?;
    Ok(())
}
