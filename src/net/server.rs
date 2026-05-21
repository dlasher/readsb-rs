use std::io;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::client::ClientConnection;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputParser {
    None,
    Beast,
    Hex,
    Sbs,
}

#[derive(Clone, Debug)]
pub struct DecodedMessage {
    pub data: Vec<u8>,
    pub client_id: u64,
}

pub struct NetworkServer {
    bind_addrs: Vec<(String, InputParser)>,
    pub message_tx: broadcast::Sender<DecodedMessage>,
    pub incoming_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    pub fn new(addrs: &[(&str, InputParser)]) -> (Self, broadcast::Receiver<DecodedMessage>) {
        let (out_tx, out_rx) = broadcast::channel(1024);
        let (in_tx, _) = broadcast::channel(1024);
        let bind_addrs = addrs.iter().map(|(a, p)| (a.to_string(), *p)).collect();
        (NetworkServer { bind_addrs, message_tx: out_tx, incoming_tx: in_tx }, out_rx)
    }

    #[allow(dead_code)] // API alternative constructor; kept for testability
    pub fn with_channel(addrs: &[(&str, InputParser)], channel: broadcast::Sender<DecodedMessage>) -> Self {
        let (in_tx, _) = broadcast::channel(1024);
        let bind_addrs = addrs.iter().map(|(a, p)| (a.to_string(), *p)).collect();
        NetworkServer { bind_addrs, message_tx: channel, incoming_tx: in_tx }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut listeners: Vec<(TcpListener, InputParser)> = Vec::new();
        for (addr, parser) in &self.bind_addrs {
            let listener = TcpListener::bind(addr).await?;
            info!("Listening on {} ({:?})", addr, parser);
            listeners.push((listener, *parser));
        }

        loop {
            let (stream, addr, parser) = match listeners.len() {
                0 => return Err(io::Error::new(io::ErrorKind::NotConnected, "no listeners")),
                1 => {
                    let (stream, addr) = listeners[0].0.accept().await?;
                    (stream, addr, listeners[0].1)
                }
                _ => {
                    let accept_futs: Vec<_> = listeners.iter().map(|(l, _)| Box::pin(l.accept())).collect();
                    let (result, idx, _) = futures::future::select_all(accept_futs).await;
                    let (stream, addr) = result?;
                    (stream, addr, listeners[idx].1)
                }
            };
            let in_tx = self.incoming_tx.clone();
            let rx = self.message_tx.subscribe();
            tokio::spawn(async move {
                warn!("Client connected: {} ({:?})", addr, parser);
                if let Err(e) = ClientConnection::handle(stream, parser, in_tx, rx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}
