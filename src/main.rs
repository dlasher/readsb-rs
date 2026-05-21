use readsb::config::ReadsbConfig;
use readsb::tracking::Tracker;
use readsb::crc::CrcFixEngine;
use readsb::sdr::{SdrManager, SdrType};
use readsb::net::NetworkServer;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = ReadsbConfig::from_cli();
    info!("Starting readsb-rs v{}", env!("CARGO_PKG_VERSION"));

    let tracker = Arc::new(Tracker::new());
    let crc_engine = Arc::new(CrcFixEngine::new(112));

    let (mut net_server, _net_rx) = NetworkServer::new(&[
        &format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_ri_port),
        &format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_bo_port),
        &format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_sbs_port),
    ]);

    // Determine input format: RTL-TCP sends U8, RTL-SDR USB sends U8, files may vary
    let input_format = match config.iformat.as_deref() {
        Some("CU8") => readsb::demod::InputFormat::U8,
        Some("SC16") => readsb::demod::InputFormat::SC16Q11,
        Some("CF32") => readsb::demod::InputFormat::F32,
        _ => {
            // Default: if using RTL-TCP, use U8; otherwise SC16Q11 for other sources
            if config.device.as_deref().map_or(false, |d| d.starts_with("rtl_tcp:")) {
                readsb::demod::InputFormat::U8
            } else {
                readsb::demod::InputFormat::SC16Q11
            }
        }
    };

    let mut sdr = SdrManager::new();
    let sdr_type = match config.ifile {
        Some(ref path) => SdrType::IFile(path.clone()),
        None => {
            if let Some(ref dev) = config.device {
                if dev.starts_with("rtl_tcp:") {
                    let parts: Vec<&str> = dev.split(':').collect();
                    if parts.len() >= 3 {
                        let host = parts[1].to_string();
                        let port: u16 = parts[2].parse().unwrap_or(1234);
                        info!("Using RTL-TCP: {}:{}", host, port);
                        SdrType::RtlTcp(host, port)
                    } else {
                        warn!("Invalid rtl_tcp format, expected rtl_tcp:host:port");
                        SdrType::RtlSdr(0)
                    }
                } else {
                    // Parse --device as a numeric device index
                    let idx: u32 = dev.parse().unwrap_or(0);
                    info!("Using RTL-SDR device index {}", idx);
                    SdrType::RtlSdr(idx)
                }
            } else {
                info!("Using RTL-SDR device index 0 (default)");
                SdrType::RtlSdr(0)
            }
        }
    };

    info!("Opening SDR device...");
    if let Err(e) = sdr.open(sdr_type).await {
        warn!("Failed to open SDR device: {}", e);
        return;
    }
    info!("SDR device opened successfully");

    let mut sample_buffer = vec![0u8; 2 * 2400000];
    let mut magnitude_buffer = vec![0u16; 2400000];

    let net_handle = tokio::spawn(async move {
        if let Err(e) = net_server.run().await {
            warn!("Network server error: {}", e);
        }
    });

    info!("Entering main processing loop");

    let mut total_messages: u64 = 0;
    let mut iter_count: u64 = 0;

    loop {
        tokio::select! {
            result = sdr.read_samples(&mut sample_buffer) => {
                match result {
                    Ok(n) if n > 0 => {
                        iter_count += 1;
                        let count = readsb::demod::convert_to_magnitude(
                            &sample_buffer[..n],
                            input_format,
                            &mut magnitude_buffer,
                        );
                        let messages = readsb::demod::demodulate2400(
                            &magnitude_buffer, count, 0,
                        );
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        for raw_msg in &messages {
                            if let Some(result) = readsb::modes::parse_modes_message(
                                raw_msg, 112, &crc_engine,
                            ) {
                                tracker.update_from_message(&result.message, now);
                                total_messages += 1;
                            }
                        }
                        if iter_count % 100 == 0 {
                            let preambles = messages.len();
                            let len = tracker.registry.len();
                            println!("[iter {} bytes={} mag={} preambles={} decodes={} aircraft={}]",
                                iter_count, n, count, preambles, total_messages, len);
                        }
                    }
                    Ok(0) => {
                        info!("End of data stream");
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!("SDR read error: {}", e);
                        break;
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Shutting down...");
                break;
            }
        }
    }

    net_handle.abort();
    info!("Shutdown complete");
}
