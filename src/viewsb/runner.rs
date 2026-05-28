use crate::viewsb::cli::ViewsbArgs;
use crate::viewsb::formatter::{format_aircraft_table, sort_aircraft};
use crate::viewsb::output::{CsvWriter, JsonWriter};
use crate::viewsb::screen::TerminalGuard;
use crate::crc::CrcFixEngine;
use crate::modes::parse_modes_message;
use crate::tracking::Tracker;
use std::io::{self, stdout, IsTerminal};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    Ok(stream)
}

pub fn run(args: ViewsbArgs) {
    let interactive = !args.no_interactive && stdout().is_terminal();
    let show_dist = args.lat.is_some() && args.lon.is_some();
    let sort_column = args.sort.unwrap_or_else(|| args.default_sort());

    let mut tracker = Tracker::new();
    if let Some(lat) = args.lat {
        tracker.user_lat = lat;
    }
    if let Some(lon) = args.lon {
        tracker.user_lon = lon;
    }

    let crc_112 = CrcFixEngine::new(112);
    let crc_56 = CrcFixEngine::new(56);

    let mut stream = connect(&args.host, args.port).unwrap_or_else(|e| {
        eprintln!("Connection failed to {}:{}: {}", args.host, args.port, e);
        std::process::exit(1);
    });

    let mut msg_count = 0usize;
    let json_writer = args.json.as_ref().map(|p| JsonWriter::new(p).unwrap());
    let mut csv_writer = args.csv.as_ref().map(|p| CsvWriter::new(p).unwrap());

    let mut screen = if interactive {
        match TerminalGuard::new() {
            Ok(guard) => Some(guard),
            Err(e) => {
                eprintln!("Failed to initialize terminal: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    loop {
        if args.count > 0 && msg_count >= args.count {
            break;
        }

        let frames = match crate::net::protocols::beast::read_beast_frames(&mut stream, 100) {
            Ok(frames) => frames,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                vec![]
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                vec![]
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        for frame in &frames {
            let msgbits = frame.payload.len() * 8;
            let crc = if msgbits == 56 { &crc_56 } else { &crc_112 };
            if let Some(result) = parse_modes_message(&frame.payload, msgbits, crc, frame.rssi as f64) {
                if result.crc_ok || result.corrected {
                    tracker.update_from_message(&result.message, now);
                    msg_count += 1;
                }
            }
        }

        tracker.remove_stale(now);

        let mut aircraft = tracker.registry.iter_aircraft();
        if !args.show_all {
            aircraft.retain(|a| a.lat != 0.0 || a.lon != 0.0);
        }

        sort_aircraft(&mut aircraft, sort_column, tracker.user_lat, tracker.user_lon);

        if interactive {
            if let Some(ref mut screen) = screen {
                if screen.should_exit().unwrap_or(false) {
                    break;
                }
                screen.clear().ok();
                let rows = format_aircraft_table(
                    &aircraft, now, args.metric, show_dist, tracker.user_lat, tracker.user_lon
                );
                for row in &rows {
                    screen.write_line(row).ok();
                }
            }
        } else {
            let rows = format_aircraft_table(
                &aircraft, now, args.metric, show_dist, tracker.user_lat, tracker.user_lon
            );
            for row in &rows {
                println!("{}", row);
            }
            if args.count > 0 && msg_count >= args.count {
                break;
            }
        }

        if let Some(ref writer) = json_writer {
            let _ = writer.write_snapshot(&aircraft, now);
        }
        if let Some(ref mut writer) = csv_writer {
            let _ = writer.write_snapshot(&aircraft, now);
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    // Final output for non-interactive mode
    if !interactive && args.count > 0 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let aircraft = tracker.registry.iter_aircraft();
        if let Some(ref writer) = json_writer {
            let _ = writer.write_snapshot(&aircraft, now);
        }
        if let Some(ref mut writer) = csv_writer {
            let _ = writer.write_snapshot(&aircraft, now);
        }
    }
}
