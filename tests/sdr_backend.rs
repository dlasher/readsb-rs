use readsb::sdr::{SdrManager, SdrType};

// Mock RTL-TCP server for testing protocol compliance
fn start_mock_rtl_tcp_server(port: u16) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        let (mut stream, _) = listener.accept().unwrap();

        // RTL-TCP protocol: server immediately sends dongle info (12 bytes)
        // magic[4] + tuner_type[4] + tuner_gain_count[4]
        use std::io::{Read, Write};
        stream.write_all(b"RTL0").unwrap();
        // tuner_type = 5, tuner_gain_count = 29 (big-endian u32)
        stream.write_all(&5u32.to_be_bytes()).unwrap();
        stream.write_all(&29u32.to_be_bytes()).unwrap();

        // Command loop: client sends 5-byte commands (1 cmd + 4 param BE)
        loop {
            let mut cmd_buf = [0u8; 5];
            if stream.read_exact(&mut cmd_buf).is_err() {
                break;
            }
            let _cmd = cmd_buf[0];
            let _param = u32::from_be_bytes([cmd_buf[1], cmd_buf[2], cmd_buf[3], cmd_buf[4]]);

            // Echo back: RTL-TCP server echoes the command bytes
            if stream.write_all(&cmd_buf).is_err() {
                break;
            }
        }
    })
}

#[tokio::test]
async fn test_mock_device_read_samples() {
    let canned = b"\x00\x08\x00\x00\x00\x00\x00\x08".to_vec();
    let mut manager = SdrManager::new();
    let result = manager.open(SdrType::Mock(canned.clone())).await;
    assert!(result.is_ok(), "Mock device should open");

    let mut buf = vec![0u8; 8];
    let n = manager.read_samples(&mut buf).await.expect("read_samples should return Ok");
    assert_eq!(n, 8);
    assert_eq!(&buf[..], &canned[..]);
}

#[tokio::test]
async fn test_rtlsdr_usb_open_no_panic() {
    let mut manager = SdrManager::new();
    // open() should not panic regardless of whether a device is present
    let result = manager.open(SdrType::RtlSdr(0)).await;
    // If a device is present, this should succeed.
    // If no device, this should return an Err (not panic).
    // Both outcomes are valid.
    if result.is_ok() {
        let mut buf = vec![0u8; 1024];
        let read_result = manager.read_samples(&mut buf).await;
        // read should not panic either — must return Ok or Err
        let _ = read_result;
    }
}

#[tokio::test]
async fn sdr_manager_creates_ifile() {
    let mut manager = SdrManager::new();
    
    // Should succeed with IFile device
    let result = manager.open(SdrType::IFile("Cargo.toml".to_string())).await;
    
    assert!(result.is_ok(), "IFile device should open successfully");
}

#[tokio::test]
async fn sdr_manager_creates_rtlsdr() {
    let mut manager = SdrManager::new();
    // With FFI wired, open() tries to open the USB device.
    // May succeed (device attached) or fail (no device) — neither panics.
    let _ = manager.open(SdrType::RtlSdr(0)).await;
    // Test passes if no panic occurred
}

#[tokio::test]
async fn sdr_manager_creates_rtl_tcp() {
    let mut manager = SdrManager::new();
    
    // Start mock server first
    let _server = start_mock_rtl_tcp_server(19001);
    
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // Should succeed with RtlTcp device
    let result = manager.open(SdrType::RtlTcp("127.0.0.1".to_string(), 19001)).await;
    
    assert!(result.is_ok(), "RtlTcp device should be created");
}

#[tokio::test]
async fn test_mock_device_set_gain() {
    let mut manager = SdrManager::new();
    manager.open(SdrType::Mock(vec![])).await.unwrap();
    let result = manager.set_gain(42.0).await;
    assert!(result.is_ok(), "set_gain should succeed on mock device");
}
