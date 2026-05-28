use readsb::config::ReadsbConfig;
use readsb::console::{ConsoleLevel, Outputter};
use readsb::tracking::Tracker;
use readsb::crc::CrcFixEngine;
use readsb::sdr::{SdrManager, SdrType};
use readsb::stats::Stats;
use readsb::net::NetworkServer;
use readsb::net::server::InputParser;
use readsb::net::protocols::beast::encode_beast_output;
use readsb::demod::{DemodConfig, InputFormat};
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

fn bytes_per_iq_pair(format: InputFormat) -> usize {
    match format {
        InputFormat::U8 => 2,
        InputFormat::SC16Q11 => 4,
        InputFormat::SC16Q11M => 4,
        InputFormat::F32 => 8,
    }
}

#[derive(Default, Debug)]
struct DiagSnapshot {
    bytes_read: usize,
    samples_processed: usize,
    preamble_candidates: u32,
    msg_count: u32,
    crc_ok: u32,
    crc_fail: u32,
    bitfix: u32,
    iter_count: u64,
}

fn diagnostic_enabled() -> bool {
    std::env::var("READSB_DIAGNOSTIC").as_deref() == Ok("1")
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
    let crc_engine_112 = Arc::new(CrcFixEngine::new(112));
    let crc_engine_56 = Arc::new(CrcFixEngine::new(56));

    let (mut net_server, _beast_rx, _hex_rx, _sbs_rx) = NetworkServer::new(&[
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_ri_port), InputParser::Hex),
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_bo_port), InputParser::Beast),
        (&format!("{}:{}", config.net_bind_address.as_deref().unwrap_or("0.0.0.0"), config.net_sbs_port), InputParser::Sbs),
    ]);
    let beast_tx = net_server.beast_tx.clone();
    let hex_tx = net_server.hex_tx.clone();
    let sbs_tx = net_server.sbs_tx.clone();
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

    let input_format_orig = input_format;
    let input_format = match input_format {
        InputFormat::SC16Q11M => InputFormat::SC16Q11,
        other => other,
    };
    let bpiq = bytes_per_iq_pair(input_format);

    let mut sdr = SdrManager::new();

    // Track if we're using RTL-TCP so we can wire up ring buffer
    let is_rtltcp = if config.ifile.is_none() {
        if let Some(ref dev) = config.device {
            dev.starts_with("rtl_tcp:")
        } else {
            false
        }
    } else {
        false
    };

    // Compute ring buffer capacity in chunks (capacity bytes / 262KB per chunk)
    let rtltcp_chunk_size = std::env::var("READSB_TCP_CHUNK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(262_144);
    let rtltcp_capacity = config.ringbuf_size / rtltcp_chunk_size;

    let read_target = match is_rtltcp {
        true => rtltcp_chunk_size,
        false => 2 * 2_400_000,
    };
    info!("SDR read target: {} bytes per call", read_target);

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
    if is_rtltcp {
        // RTL_TCP: use ring buffer to decouple TCP reader from main loop
        if let SdrType::RtlTcp(host, port) = sdr_type {
            let device = SdrManager::create_rtl_tcp_device(host, port, rtltcp_capacity, rtltcp_chunk_size);
            if let Err(e) = sdr.open_with_device(device).await {
                warn!("Failed to open SDR device: {}", e);
                return;
            }
        } else {
            warn!("RTL-TCP device type mismatch");
            return;
        }
    } else {
        if let Err(e) = sdr.open(sdr_type).await {
            warn!("Failed to open SDR device: {}", e);
            return;
        }
    }
    info!("SDR device opened successfully");

    if let Some(gain) = config.gain {
        info!("Setting gain to {} dB", gain);
        if let Err(e) = sdr.set_gain(gain).await {
            warn!("Failed to set gain: {}", e);
        }
    }

    const OVERLAP_SAMPLES: usize = 300;
    let overlap_bytes = OVERLAP_SAMPLES * bpiq;
    let mut sample_buffer = vec![0u8; read_target];
    let combined_len = read_target + overlap_bytes;
    let mut combined = vec![0u8; combined_len];
    let max_samples = combined_len / bpiq;
    let mut magnitude_buffer = vec![0u16; max_samples];
    let mut overlap_tail = vec![128u8; overlap_bytes];

    // SC16Q11M: strip 12-byte header from first read, prime overlap_tail
    if let InputFormat::SC16Q11M = input_format_orig {
        if let Ok(n) = sdr.read_samples(&mut sample_buffer).await {
            if n > 12 {
                let stripped_len = n - 12;
                sample_buffer.copy_within(12..n, 0);
                let tail_copy = overlap_bytes.min(stripped_len);
                overlap_tail.copy_within(tail_copy.., 0);
                overlap_tail[overlap_bytes - tail_copy..].copy_from_slice(
                    &sample_buffer[stripped_len - tail_copy..stripped_len],
                );
            }
        }
    }

    let net_handle = tokio::spawn(async move {
        if let Err(e) = net_server.run().await {
            warn!("Network server error: {}", e);
        }
    });

    let tracker_in = tracker.clone();
    let crc_in_112 = crc_engine_112.clone();
    let crc_in_56 = crc_engine_56.clone();
    let mut incoming_rx = incoming_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(msg) = incoming_rx.recv().await {
            let msgbits = msg.data.len() * 8;
            let engine = if msgbits == 112 { &*crc_in_112 } else { &*crc_in_56 };
            if let Some(result) = readsb::modes::parse_modes_message(
                &msg.data, msgbits, engine, 0.0,
            ) {
                if result.crc_ok {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH).unwrap_or_default()
                        .as_millis() as i64;
                    tracker_in.update_from_message(&result.message, now);
                    if result.message.addr != 0 {
                        readsb::demod::icao_filter::icao_filter_add(result.message.addr);
                    }
                }
            }
        }
    });

    // SBS output — periodic aircraft iteration (every 1s)
    let tracker_sbs = tracker.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap_or_default()
                .as_millis() as i64;
            for a in tracker_sbs.registry.iter_aircraft() {
                let sbs_data = readsb::net::protocols::sbs::encode_sbs_aircraft(&a, now);
                let _ = sbs_tx.send(sbs_data);
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

    let start_time = Instant::now();

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

    let preamble_threshold = config.preamble_threshold.unwrap_or(58);
    let agc = config.agc;
    let mut current_gain = config.gain.unwrap_or(49.6);

    let mut total_messages: u64 = 0;
    let mut stats = Stats::new();
    let mut stats_printed_at = SystemTime::now();

    let diag_enabled = diagnostic_enabled();
    let mut diag = DiagSnapshot::default();
    let mut diag_last_log = Instant::now();

    while !shutdown.load(Ordering::Relaxed) {
        let result = sdr.read_samples(&mut sample_buffer).await;
        if shutdown.load(Ordering::Relaxed) { break; }
        match result {
            Ok(n) if n > 0 => {
                // Build combined buffer: overlap tail + new data
                combined[..overlap_bytes].copy_from_slice(&overlap_tail);
                combined[overlap_bytes..overlap_bytes + n].copy_from_slice(&sample_buffer[..n]);
                let combined_len = overlap_bytes + n;

                let count = readsb::demod::convert_to_magnitude(
                    &combined[..combined_len],
                    input_format,
                    &mut magnitude_buffer,
                );

                // Save tail for next iteration: last overlap_bytes of new data
                let tail_copy = n.min(overlap_bytes);
                if tail_copy > 0 {
                    let keep = overlap_bytes - tail_copy;
                    overlap_tail.copy_within(tail_copy.., 0);
                    overlap_tail[keep..].copy_from_slice(&sample_buffer[n - tail_copy..n]);
                }
                let demod_config = DemodConfig {
                    preamble_threshold,
                    fix_df: false,
                    auto_gain: agc,
                    multi_pass: config.multi_pass,
                    multi_pass_margin: config.multi_pass_margin,
                };

                // Use multi-pass demodulation when enabled (default: true)
                let demod_result = if config.multi_pass {
                    let mut mag_copy = magnitude_buffer[..count].to_vec();
                    readsb::demod::demodulate2400_multi_pass(
                        &mut mag_copy, count, &demod_config,
                    )
                } else {
                    readsb::demod::demodulate2400_v2(
                        &magnitude_buffer[..count], count, &demod_config,
                    )
                };


                if let Some((ac_code, _spi)) = readsb::demod::demodulate_ac(&magnitude_buffer[..count]) {
                    let _ = ac_code;
                }

                if diag_enabled {
                    diag.bytes_read = n;
                    diag.samples_processed = count;
                    diag.preamble_candidates = demod_result.stats.preamble_candidates;
                    diag.msg_count = demod_result.messages.len() as u32;
                    diag.iter_count += 1;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let now_monotonic = Instant::now();

                for msg in &demod_result.messages {
                    let msgbits = msg.bytes.len() * 8;
                    let engine = if msgbits == 112 { &*crc_engine_112 } else { &*crc_engine_56 };
                    if let Some(result) = readsb::modes::parse_modes_message(
                        &msg.bytes, msgbits, engine, msg.signal,
                    ) {
                        if diag_enabled {
                            if result.corrected {
                                diag.crc_ok += 1;
                                diag.bitfix += 1;
                            } else if result.crc_ok {
                                diag.crc_ok += 1;
                            } else {
                                diag.crc_fail += 1;
                            }
                        }
                        if result.crc_ok || result.corrected {
                            tracker.update_from_message(&result.message, now);
                            if result.message.addr != 0 {
                                readsb::demod::icao_filter::icao_filter_add(result.message.addr);
                            }
                            total_messages += 1;

                            // Outputter feed — per-message console output
                            let current_level = level_from_u8(CONSOLE_LEVEL.load(Ordering::Relaxed));
                            outputter.set_level(current_level);
                            for line in outputter.feed(&result.message, now_monotonic) {
                                println!("{}", line);
                            }
                        }

                        // Always output to Beast/Hex (even CRC_FAIL) to match readsb-C
                        let timestamp_us = start_time.elapsed().as_micros() as i64;
                        let beast_data = encode_beast_output(&msg.bytes, msg.signal, timestamp_us);
                        let _ = beast_tx.send(beast_data);
                        let hex_data = readsb::net::protocols::hex::encode_hex_output(&msg.bytes);
                        let _ = hex_tx.send(hex_data);
                    }
                }
                tracker.remove_stale(now);
                stats.samples_processed += count as u64;
                stats.messages_total = total_messages as u32;
                stats.unique_aircraft = tracker.registry.len() as u32;

                // Auto-gain control
                if agc && count > 0 {
                    let s = &demod_result.stats;
                    if s.loud_events > s.noise_high_samples * 10 {
                        let new_gain = (current_gain - 1.0).max(0.0);
                        info!("AGC: reducing gain {:.1} → {:.1}", current_gain, new_gain);
                        current_gain = new_gain;
                        let _ = sdr.set_gain(current_gain).await;
                    } else if s.noise_low_samples > (count as u32 / 2) {
                        let new_gain = (current_gain + 1.0).min(50.0);
                        info!("AGC: increasing gain {:.1} → {:.1}", current_gain, new_gain);
                        current_gain = new_gain;
                        let _ = sdr.set_gain(current_gain).await;
                    }
                }

                // Periodic low/medium console output
                let now_monotonic = Instant::now();
                let tracked = tracker.registry.len() as u32;

                for line in outputter.flush_low(now_monotonic, tracked) {
                    println!("{}", line);
                }
                for line in outputter.flush_medium(now_monotonic) {
                    println!("{}", line);
                }

                // Diagnostic counters
                if diag_enabled && diag_last_log.elapsed() >= Duration::from_secs(5) {
                    let elapsed = diag_last_log.elapsed().as_secs_f64().max(0.001);
                    let reads_per_sec = diag.iter_count as f64 / elapsed;
                    eprintln!(
                        "DIAG: {}B {}samp {}pre {}msgs {}ok {}fail {}fix {:.1}rps",
                        diag.bytes_read,
                        diag.samples_processed,
                        diag.preamble_candidates,
                        diag.msg_count,
                        diag.crc_ok,
                        diag.crc_fail,
                        diag.bitfix,
                        reads_per_sec,
                    );
                    diag_last_log = Instant::now();
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
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
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
