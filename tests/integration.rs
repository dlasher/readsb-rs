use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The binary path
fn bin() -> Command {
    let mut cmd = Command::new(
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("readsb")
    );
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::piped());
    cmd
}

fn find_port() -> u16 {
    let a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = a.local_addr().unwrap().port();
    drop(a);
    port
}

/// D1: Stale removal timer runs and the binary processes messages without panic.
/// Currently expected to fail or timeout — stale removal not yet implemented.
#[test]
fn test_stale_removal_runs() {
    let port = find_port();
    let mut child = bin()
        .args(&[
            "--ifile", "test_fixtures/synthetic_df17.iq",
            "--net",
            "--net-bo-port", &port.to_string(),
            "--net-ri-port", &find_port().to_string(),
            "--net-sbs-port", &find_port().to_string(),
        ])
        .spawn()
        .expect("Failed to spawn");

    std::thread::sleep(Duration::from_secs(2));

    // Process should still be running (or exited cleanly)
    match child.try_wait() {
        Ok(Some(status)) => {
            // Exited — currently this happens because shutdown is abort()
            // TODO: After graceful shutdown, should still be running or exit 0
            eprintln!("Binary exited: {:?}", status.code());
        }
        Ok(None) => {
            // Still running — stale removal timer is active
            child.kill().ok();
        }
        Err(e) => panic!("Wait error: {}", e),
    }
}

/// D2: JSON output with --json-dir. Currently expected to not produce the file.
#[test]
fn test_json_output_dir() {
    let tmpdir = std::env::temp_dir().join(format!("readsb_d2_{}", std::process::id()));
    std::fs::create_dir_all(&tmpdir).unwrap();
    let json_dir = tmpdir.to_str().unwrap().to_string();

    let port = find_port();
    let mut child = bin()
        .args(&[
            "--ifile", "test_fixtures/synthetic_df17.iq",
            "--json-dir", &json_dir,
            "--net",
            "--net-bo-port", &port.to_string(),
            "--net-ri-port", &find_port().to_string(),
            "--net-sbs-port", &find_port().to_string(),
        ])
        .spawn()
        .expect("Failed to spawn");

    std::thread::sleep(Duration::from_secs(3));

    // Check if aircraft.json was written
    // TODO: After implementing JSON output, this should exist
    let aircraft_json = Path::new(&json_dir).join("aircraft.json");
    eprintln!("aircraft.json exists: {}", aircraft_json.exists());
    // Remove temp dir
    child.kill().ok();
    child.wait().ok();
    std::fs::remove_dir_all(&tmpdir).ok();
}

/// D3: Stats are collected during processing. Currently stats struct exists
/// but is not wired into the processing pipeline.
#[test]
fn test_stats_collected() {
    // For now, just verify the Stats struct works (unit test)
    // Integration test will be added when stats are wired
    let stats = readsb::stats::Stats::new();
    assert_eq!(stats.messages_total, 0);
    assert!(stats.distance_min > 1e30); // f64::MAX
}

/// D4: SIGTERM causes clean exit. Currently aborts.
#[test]
fn test_graceful_shutdown() {
    let port = find_port();
    let mut child = bin()
        .args(&[
            "--ifile", "test_fixtures/synthetic_df17.iq",
            "--net",
            "--net-bo-port", &port.to_string(),
            "--net-ri-port", &find_port().to_string(),
            "--net-sbs-port", &find_port().to_string(),
        ])
        .spawn()
        .expect("Failed to spawn");

    std::thread::sleep(Duration::from_millis(500));

    // Send SIGTERM via kill command
    Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .ok();

    // Wait briefly for graceful exit
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // TODO: After implementing graceful shutdown, expect exit 0
                eprintln!("Exited with: {:?}", status.code());
                break;
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(3) {
                    child.kill().ok();
                    eprintln!("Forced kill after timeout");
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("Wait error: {}", e),
        }
    }
}
