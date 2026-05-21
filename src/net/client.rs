use std::io;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::server::DecodedMessage;

pub struct ClientConnection;

impl ClientConnection {
    pub async fn handle(
        mut stream: TcpStream,
        _incoming_tx: broadcast::Sender<DecodedMessage>,
        mut outgoing_rx: broadcast::Receiver<DecodedMessage>,
    ) -> io::Result<()> {
        let (mut rx, mut tx) = stream.split();

        tokio::select! {
            result = read_loop(&mut rx) => result,
            result = write_loop(&mut tx, &mut outgoing_rx) => result,
        }
    }
}

async fn read_loop(rx: &mut tokio::net::tcp::ReadHalf<'_>) -> io::Result<()> {
    let mut buf = vec![0u8; 65536];
    loop {
        let n = rx.read(&mut buf).await?;
        if n == 0 { break; }
    }
    info!("Client disconnected (read)");
    Ok(())
}

async fn write_loop(tx: &mut tokio::net::tcp::WriteHalf<'_>, rx: &mut broadcast::Receiver<DecodedMessage>) -> io::Result<()> {
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if let Err(e) = tx.write_all(&msg.data).await {
                    warn!("Write error: {}", e);
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        }
    }
    Ok(())
}
