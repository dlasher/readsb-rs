use std::io;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;
use tracing::info;
use super::server::DecodedMessage;

pub struct ClientConnection;

impl ClientConnection {
    pub async fn handle(mut stream: TcpStream, _tx: broadcast::Sender<DecodedMessage>) -> io::Result<()> {
        let mut buf = vec![0u8; 65536];
        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 { break; }
        }
        info!("Client disconnected");
        Ok(())
    }
}
