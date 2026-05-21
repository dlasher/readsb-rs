use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Helper: find an available port
fn find_port() -> u16 {
    let a = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = a.local_addr().unwrap().port();
    drop(a);
    port
}

/// Test that the server accepts connections on both bound listeners.
/// With only listener[0] being serviced, a connection to listener[1]
/// will be accepted by the OS but never processed, causing it to hang
/// on read. After the fix, the connection should be serviced and closed
/// cleanly (read returns 0).
#[tokio::test]
async fn test_multi_listener_accept() {
    let port1 = find_port();
    let port2 = find_port();
    let addr1 = format!("127.0.0.1:{}", port1);
    let addr2 = format!("127.0.0.1:{}", port2);

    let (mut server, _rx) = readsb::net::NetworkServer::new(&[&addr1, &addr2]);

    let jh = tokio::spawn(async move {
        server.run().await.ok();
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect to port1
    let c1 = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port1)).await
        .expect("Should connect to first listener");

    // Connect to port2
    let c2 = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port2)).await
        .expect("Should connect to second listener");

    // Both TCP-level connections succeeded (kernel SYN-ACK).
    // Now detect whether the server's handler was spawned for port2.
    // After the fix, handler reads until EOF then returns, closing
    // the connection. Before the fix, handler never runs for port2.
    //
    // Detection: write data, shutdown write (sends FIN), then read.
    // If handler was spawned: read returns 0 (EOF).
    // If handler was NOT spawned: connection not closed, read hangs.
    let (mut rx2, mut tx2) = c2.into_split();
    tx2.write_all(b"hello").await.unwrap();
    drop(tx2);

    // Read with timeout — should succeed once handler processes connection
    let result = tokio::time::timeout(Duration::from_secs(3), rx2.read(&mut [0u8; 1])).await;
    match result {
        Ok(Ok(0)) => {} // Connection closed by handler — correct!
        Ok(Ok(n)) => panic!("Unexpected read of {} bytes (expected EOF=0)", n),
        Ok(Err(e)) => panic!("Read error: {}", e),
        Err(_) => panic!("TIMEOUT: second listener was not serviced"),
    }

    drop(c1);
    jh.abort();
}

/// Test that clients receive broadcast messages written to their stream.
/// We create a server, connect a client, publish via the broadcast sender,
/// and verify the client reads the data from its stream.
#[tokio::test]
async fn test_client_receives_broadcast() {
    let port = find_port();
    let addr = format!("127.0.0.1:{}", port);

    let (mut server, _rx) = readsb::net::NetworkServer::new(&[&addr]);
    let tx = server.message_tx.clone();

    let jh = tokio::spawn(async move {
        server.run().await.ok();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut client = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port)).await
        .expect("Should connect");

    // Give the server time to spawn the handler
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish a message on the broadcast channel
    let msg = readsb::net::DecodedMessage {
        data: vec![0x10, 0x03, 0x01, 0x02, 0x03],
        client_id: 0,
    };
    tx.send(msg).unwrap();

    // Client should receive the message in its stream
    let mut buf = vec![0u8; 5];
    let result = tokio::time::timeout(Duration::from_secs(1), client.read_exact(&mut buf)).await;
    match result {
        Ok(Ok(_)) => {
            assert_eq!(buf, vec![0x10, 0x03, 0x01, 0x02, 0x03]);
        }
        Ok(Err(e)) => panic!("Read error: {}", e),
        Err(_) => panic!("Timeout: client did not receive broadcast message"),
    }

    jh.abort();
}
