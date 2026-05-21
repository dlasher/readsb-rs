use std::process::Command;

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
