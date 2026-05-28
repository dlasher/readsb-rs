use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, ErrorKind, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "beast-client", version = "0.9.6")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a Beast data source and save frames to a record file
    Record {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
        #[arg(long)]
        output: String,
    },
    /// Play back a record file, printing hex or decoded messages
    Play {
        file: String,
        #[arg(long)]
        hex: bool,
        #[arg(long)]
        decode: bool,
    },
    /// Compare Beast data from record files and/or live sources (side-by-side diff)
    Compare {
        #[arg(long)]
        ref1: Option<String>,
        #[arg(long)]
        ref2: Option<String>,
        #[arg(long, default_value = "127.0.0.1")]
        host1: String,
        #[arg(long, default_value_t = 30005)]
        port1: u16,
        #[arg(long, default_value = "127.0.0.1")]
        host2: String,
        #[arg(long, default_value_t = 30006)]
        port2: u16,
        #[arg(long, default_value_t = 10)]
        window: u64,
        #[arg(long, default_value_t = 0)]
        duration: u64,
        #[arg(long)]
        output: Option<String>,
    },
    /// Connect to a Beast source and print hex-encoded messages
    Hex {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
    },
    /// Connect to a Beast source and decode Mode-S messages
    Decode {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
    },
    /// Connect to a Beast source and view live aircraft table
    Viewsb {
        #[command(flatten)]
        args: readsb::viewsb::cli::ViewsbArgs,
    },
}

fn connect(host: &str, port: u16, timeout_ms: u64) -> TcpStream {
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr).unwrap_or_else(|e| {
        eprintln!("Connection failed to {addr}: {e}");
        std::process::exit(1);
    });
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)));
    stream
}

fn read_batch(stream: &mut TcpStream) -> Vec<readsb::net::protocols::beast::BeastFrame> {
    match readsb::net::protocols::beast::read_beast_frames(stream, 1000) {
        Ok(frames) => frames,
        Err(ref e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
            vec![]
        }
        Err(e) => {
            eprintln!("Read error: {e}");
            vec![]
        }
    }
}

fn collect_frames(stream: &mut TcpStream, total_wait_ms: u64) -> Vec<readsb::net::protocols::beast::BeastFrame> {
    let interval_ms = 1000u64;
    let iterations = (total_wait_ms / interval_ms).max(1);
    for _ in 0..iterations {
        let batch = read_batch(stream);
        if !batch.is_empty() {
            return batch;
        }
    }
    vec![]
}

fn decode_frames(frames: &[readsb::net::protocols::beast::BeastFrame]) {
    let crc_112 = readsb::crc::CrcFixEngine::new(112);
    let crc_56 = readsb::crc::CrcFixEngine::new(56);
    for frame in frames {
        let msgbits = frame.payload.len() * 8;
        let crc = if msgbits == 56 { &crc_56 } else { &crc_112 };
        if let Some(result) =
            readsb::modes::parse_modes_message(&frame.payload, msgbits, crc, frame.rssi as f64)
        {
            if result.crc_ok {
                let icao = format!("{:06X}", result.message.addr);
                println!("{}\tDF{}\t{}", frame.timestamp, result.message.msgtype, icao);
            } else {
                println!("{}\tCRC_FAIL\tDF{}", frame.timestamp, result.message.msgtype as u32);
            }
        }
    }
}

fn hex_frames(frames: &[readsb::net::protocols::beast::BeastFrame]) {
    for frame in frames {
        let hex = readsb::net::protocols::hex::encode_hex_output(&frame.payload);
        print!("{}", std::str::from_utf8(&hex).unwrap_or(""));
    }
    std::io::stdout().flush().ok();
}

fn read_records_from_file(path: &str) -> Vec<readsb::net::protocols::beast::BeastFrame> {
    let data = fs::read(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });
    let mut frames = Vec::new();
    let mut i = 0;
    while i + 9 <= data.len() {
        let payload_len = data[i + 8] as usize;
        let total_len = 9 + payload_len;
        if i + total_len > data.len() {
            break;
        }
        if let Some(frame) =
            readsb::net::protocols::beast::decode_record(&data[i..i + total_len])
        {
            frames.push(frame);
        }
        i += total_len;
    }
    frames
}

fn collect_window(host: &str, port: u16, window_secs: u64) -> Vec<readsb::net::protocols::beast::BeastFrame> {
    let addr = format!("{}:{}", host, port);
    let mut stream = match TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Connection failed to {addr}: {e}");
            return vec![];
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let mut all_frames = Vec::new();
    let start = Instant::now();
    let deadline = Duration::from_secs(window_secs);
    loop {
        if start.elapsed() >= deadline {
            break;
        }
        match readsb::net::protocols::beast::read_beast_frames(&mut stream, 1000) {
            Ok(frames) => {
                all_frames.extend(frames);
                if all_frames.len() >= 1000 {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
            {
            }
            Err(e) => {
                eprintln!("Read error from {addr}: {e}");
                break;
            }
        }
    }
    all_frames
}

fn open_output(path: Option<&str>) -> Box<dyn Write> {
    match path {
        Some(p) => Box::new(BufWriter::new(File::create(p).unwrap_or_else(|e| {
            eprintln!("Failed to create {p}: {e}");
            std::process::exit(1);
        }))),
        None => Box::new(std::io::stdout()),
    }
}

fn print_window_diff(
    left_map: &HashMap<(u32, u8), Vec<u8>>,
    right_map: &HashMap<(u32, u8), Vec<u8>>,
    writer: &mut dyn Write,
) -> (usize, usize, usize, usize, usize) {
    let mut matched = 0usize;
    let mut matched_varies = 0usize;
    let mut diff = 0usize;
    let mut left_only = 0usize;
    let mut right_only = 0usize;

    let mut keys: Vec<&(u32, u8)> = left_map.keys().collect();
    for k in right_map.keys() {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys.sort();

    for key in &keys {
        let left_payload = left_map.get(key);
        let right_payload = right_map.get(key);
        match (left_payload, right_payload) {
            (Some(l), Some(r)) if l == r => {
                matched += 1;
            }
            (Some(_l), Some(_r))
                if key.1 == 17 || key.1 == 18 || key.1 == 19 =>
            {
                matched_varies += 1;
            }
            (Some(l), Some(r)) => {
                diff += 1;
                let left_str = format!("  {}", readsb::net::protocols::beast::format_line(key, l));
                let right_str = readsb::net::protocols::beast::format_line(key, r);
                writeln!(writer, "{:<51}|  {}", left_str, right_str).ok();
            }
            (None, Some(r)) => {
                right_only += 1;
                let right_str = readsb::net::protocols::beast::format_line(key, r);
                writeln!(writer, "{:>60}>  {}", "", right_str).ok();
            }
            (Some(l), None) => {
                left_only += 1;
                let left_str = format!("  {}", readsb::net::protocols::beast::format_line(key, l));
                writeln!(writer, "{:<60}<", left_str).ok();
            }
            (None, None) => unreachable!(),
        }
    }
    (matched, matched_varies, diff, left_only, right_only)
}

fn diff_live(
    left_host: &str,
    left_port: u16,
    right_host: &str,
    right_port: u16,
    window_secs: u64,
    duration_secs: u64,
    writer: &mut dyn Write,
) {
    let mut elapsed_secs = 0u64;
    let mut cum_matched = 0usize;
    let mut cum_varies = 0usize;
    let mut cum_diff = 0usize;
    let mut cum_left_only = 0usize;
    let mut cum_right_only = 0usize;

    loop {
        if duration_secs > 0 && elapsed_secs >= duration_secs {
            break;
        }
        let left_host_owned = left_host.to_string();
        let right_host_owned = right_host.to_string();
        let running = Arc::new(AtomicBool::new(true));
        // Print initial tick before spawning threads to guarantee ordering
        eprint!("0");
        std::io::stderr().flush().ok();
        let progress_flag = running.clone();
        let progress_handle = std::thread::spawn(move || {
            let start = Instant::now();
            loop {
                if !progress_flag.load(Ordering::Relaxed) {
                    let final_elapsed = start.elapsed().as_secs().min(window_secs);
                    eprintln!("\r{}", tick_line(final_elapsed));
                    break;
                }
                let elapsed = start.elapsed().as_secs().min(window_secs);
                eprint!("\r{}", tick_line(elapsed));
                std::io::stderr().flush().ok();
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        let left_handle = std::thread::spawn(move || {
            collect_window(&left_host_owned, left_port, window_secs)
        });
        let right_handle = std::thread::spawn(move || {
            collect_window(&right_host_owned, right_port, window_secs)
        });
        let left_frames = left_handle.join().unwrap_or_default();
        let right_frames = right_handle.join().unwrap_or_default();
        running.store(false, Ordering::Relaxed);
        let _ = progress_handle.join();

        let left_map = readsb::net::protocols::beast::build_key_map(&left_frames);
        let right_map = readsb::net::protocols::beast::build_key_map(&right_frames);

        let start_sec = elapsed_secs;
        elapsed_secs += window_secs;
        let end_sec = elapsed_secs.min(duration_secs);

        writeln!(
            writer,
            "=== Window {}-{}s (L: {}, R: {}) ===",
            start_sec,
            end_sec,
            left_frames.len(),
            right_frames.len()
        ).ok();

        if left_frames.is_empty() && right_frames.is_empty() {
            writeln!(writer, "(no frames)").ok();
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }

        let (matched, matched_varies, diff, left_only, right_only) =
            print_window_diff(&left_map, &right_map, writer);
        writeln!(
            writer,
            "--- Matched: {}, Matched-varying: {}, Diff: {}, Left-only: {}, Right-only: {} ---",
            matched, matched_varies, diff, left_only, right_only
        ).ok();

        cum_matched += matched;
        cum_varies += matched_varies;
        cum_diff += diff;
        cum_left_only += left_only;
        cum_right_only += right_only;

        writer.flush().ok();
        std::thread::sleep(Duration::from_millis(200));
    }

    if cum_matched > 0 || cum_varies > 0 || cum_diff > 0 || cum_left_only > 0 || cum_right_only > 0 {
        writeln!(
            writer,
            "\n=== Final: Matched: {}, Matched-varying: {}, Diff: {}, Left-only: {}, Right-only: {} ===",
            cum_matched, cum_varies, cum_diff, cum_left_only, cum_right_only
        ).ok();
    }
}

fn diff_file_live(
    left_frames: Vec<readsb::net::protocols::beast::BeastFrame>,
    right_host: &str,
    right_port: u16,
    window_secs: u64,
    duration_secs: u64,
    writer: &mut dyn Write,
) {
    let left_map = readsb::net::protocols::beast::build_key_map(&left_frames);
    let mut elapsed_secs = 0u64;
    let mut cum_matched = 0usize;
    let mut cum_varies = 0usize;
    let mut cum_diff = 0usize;
    let mut cum_left_only = 0usize;
    let mut cum_right_only = 0usize;

    loop {
        if duration_secs > 0 && elapsed_secs >= duration_secs {
            break;
        }
        let right_frames = collect_window(right_host, right_port, window_secs);
        let right_map = readsb::net::protocols::beast::build_key_map(&right_frames);

        let start_sec = elapsed_secs;
        elapsed_secs += window_secs;
        let end_sec = elapsed_secs.min(duration_secs);

        writeln!(
            writer,
            "=== Window {}-{}s (L: file[{}], R: {}) ===",
            start_sec,
            end_sec,
            left_frames.len(),
            right_frames.len()
        ).ok();

        if right_frames.is_empty() {
            writeln!(writer, "(no right frames)").ok();
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }

        let (matched, matched_varies, diff, left_only, right_only) =
            print_window_diff(&left_map, &right_map, writer);
        writeln!(
            writer,
            "--- Matched: {}, Matched-varying: {}, Diff: {}, Left-only: {}, Right-only: {} ---",
            matched, matched_varies, diff, left_only, right_only
        ).ok();

        cum_matched += matched;
        cum_varies += matched_varies;
        cum_diff += diff;
        cum_left_only += left_only;
        cum_right_only += right_only;

        writer.flush().ok();
        std::thread::sleep(Duration::from_millis(200));
    }

    if cum_matched > 0 || cum_varies > 0 || cum_diff > 0 || cum_left_only > 0 || cum_right_only > 0 {
        writeln!(
            writer,
            "\n=== Final: Matched: {}, Matched-varying: {}, Diff: {}, Left-only: {}, Right-only: {} ===",
            cum_matched, cum_varies, cum_diff, cum_left_only, cum_right_only
        ).ok();
    }
}

fn tick_line(elapsed_secs: u64) -> String {
    let mut s = String::from("0");
    for tick in (10..=elapsed_secs).step_by(10) {
        s.push_str("....");
        use std::fmt::Write;
        write!(s, "{}", tick).unwrap();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_line_zero() {
        assert_eq!(tick_line(0), "0");
    }

    #[test]
    fn test_tick_line_before_first_tick() {
        assert_eq!(tick_line(5), "0");
    }

    #[test]
    fn test_tick_line_first_tick() {
        assert_eq!(tick_line(10), "0....10");
    }

    #[test]
    fn test_tick_line_midpoint_between_ticks() {
        assert_eq!(tick_line(15), "0....10");
    }

    #[test]
    fn test_tick_line_second_tick() {
        assert_eq!(tick_line(20), "0....10....20");
    }

    #[test]
    fn test_tick_line_multiple_ticks() {
        assert_eq!(tick_line(35), "0....10....20....30");
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Hex { host, port } => {
            let mut stream = connect(&host, port, 1000);
            let frames = collect_frames(&mut stream, 30000);
            hex_frames(&frames);
        }
        Commands::Decode { host, port } => {
            let mut stream = connect(&host, port, 1000);
            let frames = collect_frames(&mut stream, 30000);
            decode_frames(&frames);
        }
        Commands::Viewsb { args } => {
            readsb::viewsb::run(args);
        }
        Commands::Record { host, port, output } => {
            let mut stream = connect(&host, port, 1000);
            let mut outfile = File::create(&output).unwrap_or_else(|e| {
                eprintln!("Failed to create {output}: {e}");
                std::process::exit(1);
            });
            loop {
                let frames = read_batch(&mut stream);
                if frames.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    continue;
                }
                for frame in &frames {
                    let record = readsb::net::protocols::beast::encode_record(frame);
                    outfile.write_all(&record).ok();
                }
            }
        }
        Commands::Play { file, hex, decode } => {
            let frames = read_records_from_file(&file);
            if hex {
                hex_frames(&frames);
            }
            if decode {
                decode_frames(&frames);
            }
            if !hex && !decode {
                eprintln!("No output format specified (use --hex or --decode)");
            }
        }
        Commands::Compare { ref1, ref2, host1, port1, host2, port2, window, duration, output } => {
            let left_frames: Option<Vec<readsb::net::protocols::beast::BeastFrame>> =
                ref1.as_ref().map(|f| read_records_from_file(f));
            let right_frames: Option<Vec<readsb::net::protocols::beast::BeastFrame>> =
                ref2.as_ref().map(|f| read_records_from_file(f));
            let mut writer = open_output(output.as_deref());

            match (left_frames, right_frames) {
                (Some(f1), Some(f2)) => {
                    let analysis1 = readsb::net::protocols::beast::beast_analysis(&f1);
                    writeln!(writer, "=== {} ===", ref1.as_deref().unwrap_or("left")).ok();
                    for line in readsb::net::protocols::beast::analysis_summary(&analysis1) {
                        writeln!(writer, "{line}").ok();
                    }
                    let analysis2 = readsb::net::protocols::beast::beast_analysis(&f2);
                    writeln!(writer, "\n=== {} ===", ref2.as_deref().unwrap_or("right")).ok();
                    for line in readsb::net::protocols::beast::analysis_summary(&analysis2) {
                        writeln!(writer, "{line}").ok();
                    }
                }
                (Some(f1), None) => {
                    diff_file_live(f1, &host2, port2, window, duration, &mut writer);
                }
                (None, Some(f2)) => {
                    diff_file_live(f2, &host1, port1, window, duration, &mut writer);
                }
                (None, None) => {
                    diff_live(&host1, port1, &host2, port2, window, duration, &mut writer);
                }
            }
        }
    }
}
