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
