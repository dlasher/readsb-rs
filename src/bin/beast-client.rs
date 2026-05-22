use clap::{Parser, Subcommand};
use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::net::TcpStream;

#[derive(Parser)]
#[command(name = "beast-client", version = "0.7.0")]
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
    /// Compare Beast data from record files and/or live source
    Compare {
        #[arg(long)]
        ref1: String,
        #[arg(long)]
        ref2: Option<String>,
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
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
    /// Live Mode-S decoding with continuous output
    Live {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
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
        Commands::Live { host, port } => {
            let mut stream = connect(&host, port, 1000);
            loop {
                let frames = read_batch(&mut stream);
                if frames.is_empty() {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    continue;
                }
                decode_frames(&frames);
            }
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
        Commands::Compare { ref1, ref2, host, port } => {
            let frames1 = read_records_from_file(&ref1);
            let analysis1 = readsb::net::protocols::beast::beast_analysis(&frames1);
            println!("=== {} ===", ref1);
            for line in readsb::net::protocols::beast::analysis_summary(&analysis1) {
                println!("{line}");
            }

            if let Some(ref2_path) = &ref2 {
                let frames2 = read_records_from_file(ref2_path);
                let analysis2 = readsb::net::protocols::beast::beast_analysis(&frames2);
                println!("\n=== {} ===", ref2_path);
                for line in readsb::net::protocols::beast::analysis_summary(&analysis2) {
                    println!("{line}");
                }
            }

            if let Ok(mut stream) = TcpStream::connect(format!("{host}:{port}")) {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(2000)));
                let live_frames = read_batch(&mut stream);
                if !live_frames.is_empty() {
                    let live_analysis =
                        readsb::net::protocols::beast::beast_analysis(&live_frames);
                    println!("\n=== live: {host}:{port} ===");
                    for line in readsb::net::protocols::beast::analysis_summary(&live_analysis) {
                        println!("{line}");
                    }
                }
            }
        }
    }
}
