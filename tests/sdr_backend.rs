use readsb::sdr::{SdrManager, SdrType};

// Mock RTL-TCP server for testing protocol compliance
fn start_mock_rtl_tcp_server(port: u16) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        let (mut stream, _) = listener.accept().unwrap();

        // Handshake: client sends 0x00, we respond with 4-byte response
        use std::io::{Read, Write};
        let mut buf = [0u8; 1];
        stream.read_exact(&mut buf).unwrap();
        // Response: magic (0x00), version (0x01), writable (0x00), tuner (0x01)
        stream.write_all(&[0x00, 0x01, 0x00, 0x01]).unwrap();

        // Command loop
        loop {
            let mut cmd_buf = [0u8; 5];
            if stream.read_exact(&mut cmd_buf).is_err() {
                break;
            }
            let cmd = cmd_buf[0];
            let _param = u32::from_be_bytes([cmd_buf[1], cmd_buf[2], cmd_buf[3], cmd_buf[4]]);

            // Echo response: 0x01 = success, then 3 bytes padding
            if matches!(cmd, 0x01..=0x0A | 0x0E) {
                stream.write_all(&[0x01, 0x00, 0x00, 0x00]).unwrap();
            } else {
                stream.write_all(&[0xFF, 0x00, 0x00, 0x00]).unwrap();
            }
        }
    })
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
    
    // Should succeed with RtlSdr device (even if open() is unimplemented for now)
    let result = manager.open(SdrType::RtlSdr(0)).await;
    
    assert!(result.is_ok(), "RtlSdr device should be created");
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
