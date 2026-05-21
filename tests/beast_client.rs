use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::io::Write;

#[test]
fn test_beast_client_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "beast-client", "--", "--version"])
        .output()
        .expect("Failed to run beast-client --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.7.0"), "Version should match crate version");
}

#[test]
fn test_beast_client_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--bin", "beast-client", "--", "--help"])
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
    thread::sleep(std::time::Duration::from_millis(100));

    let output = Command::new("cargo")
        .args(["run", "--bin", "beast-client", "--", "hex", "--port", &port.to_string()])
        .output().expect("Failed to run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("*8D4840D6202CC3;\n") || stdout.contains("8D4840D6202CC3"),
        "Hex output should contain frame payload: got '{}'", stdout);
}
