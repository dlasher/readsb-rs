use clap::{Parser, Subcommand};
use std::io::Write;

#[derive(Parser)]
#[command(name = "beast-client", version = "0.7.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Record {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
        #[arg(long)]
        output: String,
    },
    Play {
        file: String,
        #[arg(long)]
        hex: bool,
        #[arg(long)]
        decode: bool,
    },
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
    Hex {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
    },
    Decode {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
    },
    Live {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        #[arg(long, default_value_t = 30005)]
        port: u16,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Record { host, port, output } => {
            eprintln!("record not implemented: {host}:{port} -> {output}");
        }
        Commands::Play { file, hex, decode } => {
            eprintln!("play not implemented: {file} hex={hex} decode={decode}");
        }
        Commands::Compare { ref1, ref2, host, port } => {
            eprintln!("compare not implemented: {ref1} vs {:?} (live={host}:{port})", ref2);
        }
        Commands::Hex { host, port } => {
            let addr = format!("{}:{}", host, port);
            match std::net::TcpStream::connect(&addr) {
                Ok(mut stream) => {
                    loop {
                        match readsb::net::protocols::beast::read_beast_frames(&mut stream, 100) {
                            Ok(frames) => {
                                if frames.is_empty() { break; }
                                for frame in &frames {
                                    let hex = readsb::net::protocols::hex::encode_hex_output(&frame.payload);
                                    print!("{}", std::str::from_utf8(&hex).unwrap_or(""));
                                }
                                std::io::stdout().flush().ok();
                            }
                            Err(e) => { eprintln!("Read error: {e}"); break; }
                        }
                    }
                }
                Err(e) => { eprintln!("Connection failed: {e}"); std::process::exit(1); }
            }
        }
        Commands::Decode { host, port } => {
            eprintln!("decode not implemented: {host}:{port}");
        }
        Commands::Live { host, port } => {
            eprintln!("live not implemented: {host}:{port}");
        }
    }
}
