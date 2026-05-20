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
    message_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    pub fn new(addrs: &[&str]) -> (Self, broadcast::Receiver<DecodedMessage>) {
        let (tx, rx) = broadcast::channel(1024);
        (NetworkServer { bind_addrs: addrs.iter().map(|s| s.to_string()).collect(), message_tx: tx }, rx)
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut listeners = Vec::new();
        for addr in &self.bind_addrs {
            let listener = TcpListener::bind(addr).await?;
            info!("Listening on {}", addr);
            listeners.push(listener);
        }

        loop {
            let (stream, addr) = tokio::select! {
                result = listeners[0].accept() => result?,
            };
            let tx = self.message_tx.clone();
            tokio::spawn(async move {
                warn!("Client connected: {}", addr);
                if let Err(e) = ClientConnection::handle(stream, tx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}
