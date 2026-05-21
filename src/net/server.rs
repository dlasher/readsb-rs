use std::io;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::client::ClientConnection;

#[derive(Clone, Debug)]
pub struct DecodedMessage {
    pub data: Vec<u8>,
    pub client_id: u64,
}

pub struct NetworkServer {
    bind_addrs: Vec<String>,
    pub message_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    pub fn new(addrs: &[&str]) -> (Self, broadcast::Receiver<DecodedMessage>) {
        let (tx, rx) = broadcast::channel(1024);
        (NetworkServer { bind_addrs: addrs.iter().map(|s| s.to_string()).collect(), message_tx: tx }, rx)
    }

    pub fn with_channel(addrs: &[&str], channel: broadcast::Sender<DecodedMessage>) -> Self {
        NetworkServer { bind_addrs: addrs.iter().map(|s| s.to_string()).collect(), message_tx: channel }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut listeners = Vec::new();
        for addr in &self.bind_addrs {
            let listener = TcpListener::bind(addr).await?;
            info!("Listening on {}", addr);
            listeners.push(listener);
        }

        loop {
            let (stream, addr) = match listeners.len() {
                0 => return Err(io::Error::new(io::ErrorKind::NotConnected, "no listeners")),
                1 => listeners[0].accept().await?,
                _ => accept_any(&listeners).await?,
            };
            let tx = self.message_tx.clone();
            let rx = self.message_tx.subscribe();
            tokio::spawn(async move {
                warn!("Client connected: {}", addr);
                if let Err(e) = ClientConnection::handle(stream, tx, rx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}

async fn accept_any(listeners: &[TcpListener]) -> io::Result<(tokio::net::TcpStream, std::net::SocketAddr)> {
    futures::future::select_all(listeners.iter().map(|l| Box::pin(l.accept()))).await.0
}
