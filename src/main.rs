use readsb::config::ReadsbConfig;
use readsb::console::{ConsoleLevel, Outputter};
use readsb::tracking::Tracker;
use readsb::crc::CrcFixEngine;
use readsb::sdr::{SdrManager, SdrType};
use readsb::stats::Stats;
use readsb::net::{NetworkServer, DecodedMessage};
use readsb::net::server::InputParser;
use readsb::net::protocols::beast::encode_beast_output;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

static CONSOLE_LEVEL: AtomicU8 = AtomicU8::new(0);

fn console_level_from_env() -> ConsoleLevel {
    match std::env::var("CONSOLE_LEVEL").as_deref() {
        Ok("low") => ConsoleLevel::Low,
        Ok("medium") => ConsoleLevel::Medium,
        Ok("high") => ConsoleLevel::High,
        Ok("max") => ConsoleLevel::Max,
        _ => ConsoleLevel::Low,
    }
}

fn console_interval_from_env() -> u64 {
    std::env::var("CONSOLE_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

fn level_to_u8(level: ConsoleLevel) -> u8 {
    level as u8
}

fn level_from_u8(v: u8) -> ConsoleLevel {
    match v {
        0 => ConsoleLevel::Low,
        1 => ConsoleLevel::Medium,
        2 => ConsoleLevel::High,
        _ => ConsoleLevel::Max,
    }
}

fn cycle_level_u8(current: u8, forward: bool) -> u8 {
    match (current, forward) {
        (0, true) => 1,
        (1, true) => 2,
        (2, true) => 3,
        (3, true) => 0,
        (0, false) => 3,
        (3, false) => 2,
        (2, false) => 1,
        (1, false) => 0,
        _ => 0,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = ReadsbConfig::from_cli();
    info!("Starting readsb-rs v{}", env!("CARGO_PKG_VERSION"));

    // Set initial console level
    let initial_level = console_level_from_env();
    CONSOLE_LEVEL.store(level_to_u8(initial_level), Ordering::Relaxed);

    let mut t = Tracker::new();
    t.user_lat = config.lat.unwrap_or(0.0);
    t.user_lon = config.lon.unwrap_or(0.0);
    let tracker = Arc::new(t);
    let crc_engine = Arc::new(CrcFixEngine::new(112));

    let (mut net_server, _net_rx) = NetworkServer::new(&[
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_ri_port), InputParser::Hex),
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_bo_port), InputParser::Beast),
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_sbs_port), InputParser::Sbs),
    ]);
    let message_tx = net_server.message_tx.clone();
    let incoming_tx = net_server.incoming_tx.clone();

    let input_format = match config.iformat.as_deref() {
        Some("CU8") => readsb::demod::InputFormat::U8,
        Some("SC16") => readsb::demod::InputFormat::SC16Q11,
        Some("CF32") => readsb::demod::InputFormat::F32,
        _ => {
            if config.ifile.is_some() {
                readsb::demod::InputFormat::SC16Q11
            } else {
                readsb::demod::InputFormat::U8
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

    if let Some(gain) = config.gain {
        info!("Setting gain to {} dB", gain);
        if let Err(e) = sdr.set_gain(gain).await {
            warn!("Failed to set gain: {}", e);
        }
    }

    let mut sample_buffer = vec![0u8; 2 * 2400000];
    let mut magnitude_buffer = vec![0u16; 2400000];

    let net_handle = tokio::spawn(async move {
        if let Err(e) = net_server.run().await {
            warn!("Network server error: {}", e);
        }
    });

    let tracker_in = tracker.clone();
    let crc_in = crc_engine.clone();
    let mut incoming_rx = incoming_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = incoming_rx.recv().await {
            let msgbits = msg.data.len() * 8;
            if let Some(result) = readsb::modes::parse_modes_message(
                &msg.data, msgbits, &crc_in, 0.0,
            ) {
                if result.crc_ok {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH).unwrap_or_default()
                        .as_millis() as i64;
                    tracker_in.update_from_message(&result.message, now);
                }
            }
        }
    });

    if let Some(ref json_dir) = config.json_dir {
        let dir = json_dir.clone();
        let tracker_json = tracker.clone();
        let interval = config.json_reliable.unwrap_or(1).max(1) as u64;
        let with_globe = config.json_globe_index;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(interval * 1000));
            loop {
                tick.tick().await;
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let aircraft = tracker_json.registry.iter_aircraft();
                let json = readsb::output::json::generate_aircraft_json(aircraft, now);
                let path = format!("{}/aircraft.json", dir);
                if let Err(e) = std::fs::write(&path, json) {
                    warn!("Failed to write aircraft.json: {}", e);
                }
                if with_globe {
                    let aircraft = tracker_json.registry.iter_aircraft();
                    for a in &aircraft {
                        if a.lat != 0.0 || a.lon != 0.0 {
                            let idx = readsb::output::globe::globe_index(a.lat, a.lon);
                            let gpath = format!("{}/globe_{}.json", dir, idx);
                            let entry = readsb::output::json::generate_aircraft_json(
                                vec![a.clone()], now
                            );
                            if let Err(e) = std::fs::write(&gpath, entry) {
                                warn!("Failed to write globe JSON: {}", e);
                            }
                        }
                    }
                }
            }
        });
    }

    info!("Entering main processing loop");

    // Console outputter
    let medium_interval = console_interval_from_env();
    let mut outputter = Outputter::new(initial_level, medium_interval);

    // Signal handlers
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signals = shutdown.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        info!("Shutting down...");
        shutdown_signals.store(true, Ordering::Relaxed);
    });

    // SIGUSR1/SIGUSR2 for console level cycling
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sigusr1) = signal(SignalKind::user_defined1()) {
            tokio::spawn(async move {
                loop {
                    sigusr1.recv().await;
                    let cur = CONSOLE_LEVEL.load(Ordering::Relaxed);
                    let next = cycle_level_u8(cur, true);
                    CONSOLE_LEVEL.store(next, Ordering::Relaxed);
                    info!("Console level: {:?}", level_from_u8(next));
                }
            });
        }
        if let Ok(mut sigusr2) = signal(SignalKind::user_defined2()) {
            tokio::spawn(async move {
                loop {
                    sigusr2.recv().await;
                    let cur = CONSOLE_LEVEL.load(Ordering::Relaxed);
                    let next = cycle_level_u8(cur, false);
                    CONSOLE_LEVEL.store(next, Ordering::Relaxed);
                    info!("Console level: {:?}", level_from_u8(next));
                }
            });
        }
    }

    let mut total_messages: u64 = 0;
    let mut stats = Stats::new();
    let mut stats_printed_at = SystemTime::now();

    while !shutdown.load(Ordering::Relaxed) {
        let result = sdr.read_samples(&mut sample_buffer).await;
        if shutdown.load(Ordering::Relaxed) { break; }
        match result {
            Ok(n) if n > 0 => {
                let count = readsb::demod::convert_to_magnitude(
                    &sample_buffer[..n],
                    input_format,
                    &mut magnitude_buffer,
                );
                let messages = readsb::demod::demodulate2400(
                    &magnitude_buffer, count, 0,
                );
                if let Some((ac_code, _spi)) = readsb::demod::demodulate_ac(&magnitude_buffer) {
                    let _ = ac_code;
                }
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let now_monotonic = Instant::now();

                for (raw_msg, signal) in &messages {
                    let msgbits = raw_msg.len() * 8;
                    if let Some(result) = readsb::modes::parse_modes_message(
                        raw_msg, msgbits, &crc_engine, *signal,
                    ) {
                        if result.crc_ok {
                            tracker.update_from_message(&result.message, now);
                            total_messages += 1;

                            // Outputter feed — per-message console output
                            let current_level = level_from_u8(CONSOLE_LEVEL.load(Ordering::Relaxed));
                            outputter.set_level(current_level);
                            for line in outputter.feed(&result.message, now_monotonic) {
                                println!("{}", line);
                            }

                            let beast_data = encode_beast_output(raw_msg, *signal);
                            let _ = message_tx.send(DecodedMessage {
                                data: beast_data,
                                client_id: 0,
                            });
                        }
                    }
                }
                tracker.remove_stale(now);
                stats.samples_processed += count as u64;
                stats.messages_total = total_messages as u32;
                stats.unique_aircraft = tracker.registry.len() as u32;

                // Periodic low/medium console output
                let now_monotonic = Instant::now();
                let tracked = tracker.registry.len() as u32;

                for line in outputter.flush_low(now_monotonic, tracked) {
                    println!("{}", line);
                }
                for line in outputter.flush_medium(now_monotonic) {
                    println!("{}", line);
                }

                // Legacy stats — keep for backward compat (can be removed later)
                if stats_printed_at.elapsed().unwrap_or_default().as_secs() >= 60 {
                    info!("Stats: {} msgs, {} a/c, {} samples",
                        stats.messages_total, stats.unique_aircraft, stats.samples_processed);
                    stats_printed_at = SystemTime::now();
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

    // Final stats on shutdown
    let tracked = tracker.registry.len() as u32;
    for line in outputter.flush_low_final(tracked) {
        println!("{}", line);
    }

    net_handle.abort();
    info!("Shutdown complete");
}
