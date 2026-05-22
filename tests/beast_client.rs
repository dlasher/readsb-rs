use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::io::Write;

fn beast_client() -> Command {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("beast-client");
    Command::new(dir)
}

fn wait_for_binary() {
    // Give the system a moment for TCP teardown
    thread::sleep(std::time::Duration::from_millis(50));
}

#[test]
fn test_beast_client_cli_version() {
    let output = beast_client()
        .arg("--version")
        .output()
        .expect("Failed to run beast-client --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.7.0"), "Version should match crate version");
}

#[test]
fn test_beast_client_cli_help() {
    let output = beast_client()
        .arg("--help")
        .output()
        .expect("Failed to run beast-client --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("record"), "Help should list record command");
    assert!(stdout.contains("hex"), "Help should list hex command");
    assert!(stdout.contains("decode"), "Help should list decode command");
    assert!(stdout.contains("live"), "Help should list live command");
}

#[test]
fn test_beast_client_hex() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut buf = Vec::new();
    buf.push(0x1a); buf.push(0x32);
    buf.extend_from_slice(&[0u8; 6]);
    buf.push(0xff);
    buf.extend_from_slice(&[0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3]);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(&buf);
        }
    });
    wait_for_binary();

    let output = beast_client()
        .args(["hex", "--port", &port.to_string()])
        .output().expect("Failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("*8D4840D6202CC3;\n") || stdout.contains("8D4840D6202CC3"),
        "Hex output should contain frame payload: got '{}'", stdout);
}

#[test]
fn test_beast_client_decode() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let mut buf = Vec::new();
    buf.push(0x1a); buf.push(0x33);
    buf.extend_from_slice(&[0u8; 6]);
    buf.push(0x42);
    buf.extend_from_slice(&[0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3, 0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98]);

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.write_all(&buf);
        }
    });
    wait_for_binary();

    let output = beast_client()
        .args(["decode", "--port", &port.to_string()])
        .output().expect("Failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should parse DF17 ADS-B message and output the ICAO
    assert!(stdout.contains("4840D6"), "Should contain ICAO 4840D6: got '{}'", stdout);
}

#[test]
fn test_compare_cli_new_flags() {
    // Use localhost + high ports that will fail fast with ECONNREFUSED.
    // This tests that CLI argument parsing succeeds; the binary will try to
    // connect and fail, but that's fine — we only care about arg acceptance.
    let output = beast_client()
        .args(["compare", "--host1", "127.0.0.1", "--port1", "1",
                "--host2", "127.0.0.1", "--port2", "2",
                "--window", "5", "--duration", "30"])
        .output().expect("Failed to run");
    // The command should accept args, attempt connections (ECONNREFUSED quickly on localhost),
    // and exit without crashing. We check for success — failure here means a CLI bug.
    assert!(output.status.success(),
        "compare with new flags should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr));
}

// --- build_key_map tests ---

#[test]
fn test_build_key_map_empty() {
    let frames: Vec<readsb::net::protocols::beast::BeastFrame> = vec![];
    let result = readsb::net::protocols::beast::build_key_map(&frames);
    assert!(result.is_empty(), "Empty frames should give empty map");
}

#[test]
fn test_build_key_map_single() {
    use readsb::net::protocols::beast::BeastFrame;
    let frame = BeastFrame {
        timestamp: 0,
        frame_type: 0x32,
        payload: vec![0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3],
        rssi: 0xff,
    };
    let result = readsb::net::protocols::beast::build_key_map(&[frame]);
    assert_eq!(result.len(), 1, "Single frame should produce one entry");
    let key = (0x4840D6, 17u8); // ICAO=4840D6, DF=17 (0x8D >> 3)
    assert!(result.contains_key(&key), "Should contain ICAO=4840D6 DF=17");
    assert_eq!(result[&key].len(), 7, "Payload should be 7 bytes for short frame");
}

#[test]
fn test_build_key_map_duplicate_keeps_last() {
    use readsb::net::protocols::beast::BeastFrame;
    let frame1 = BeastFrame {
        timestamp: 0, frame_type: 0x32, rssi: 0xff,
        payload: vec![0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3],
    };
    let frame2 = BeastFrame {
        timestamp: 1, frame_type: 0x32, rssi: 0xff,
        payload: vec![0x8D, 0x48, 0x40, 0xD6, 0xAA, 0xBB, 0xCC],
    };
    let result = readsb::net::protocols::beast::build_key_map(&[frame1, frame2]);
    assert_eq!(result.len(), 1, "Duplicate ICAO+DF should produce one entry");
    let key = (0x4840D6, 17u8);
    assert_eq!(result[&key][6], 0xCC, "Should have last payload's trailing byte");
}

#[test]
fn test_build_key_map_df_discrimination() {
    use readsb::net::protocols::beast::BeastFrame;
    let frame1 = BeastFrame {
        timestamp: 0, frame_type: 0x32, rssi: 0xff,
        payload: vec![0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3],
    };
    let frame2 = BeastFrame {
        timestamp: 0, frame_type: 0x32, rssi: 0xff,
        payload: vec![0x5D, 0x48, 0x40, 0xD6, 0xBC, 0xE0],
    };
    let result = readsb::net::protocols::beast::build_key_map(&[frame1, frame2]);
    assert_eq!(result.len(), 2, "Same ICAO, different DF -> two entries");
}

#[test]
fn test_format_hex_112bit() {
    let payload = vec![0x8D, 0x48, 0x40, 0xD6, 0x20, 0x2C, 0xC3,
                       0x71, 0xC3, 0x2C, 0xE0, 0x57, 0x60, 0x98];
    let hex = readsb::net::protocols::beast::format_hex(&payload);
    assert_eq!(hex.len(), 28, "14 bytes = 28 hex chars");
    assert_eq!(&hex[..2], "8D", "First byte should be 8D");
    assert!(hex.contains("4840D6"), "Should contain ICAO");
}
