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
    pub beast_tx: broadcast::Sender<Vec<u8>>,
    pub hex_tx: broadcast::Sender<Vec<u8>>,
    pub sbs_tx: broadcast::Sender<Vec<u8>>,
    pub incoming_tx: broadcast::Sender<DecodedMessage>,
}

impl NetworkServer {
    #[allow(clippy::type_complexity)]
    pub fn new(addrs: &[(&str, InputParser)]) -> (Self, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>, broadcast::Receiver<Vec<u8>>) {
        let (beast_tx, beast_rx) = broadcast::channel(1024);
        let (hex_tx, hex_rx) = broadcast::channel(1024);
        let (sbs_tx, sbs_rx) = broadcast::channel(1024);
        let (in_tx, _) = broadcast::channel(1024);
        let bind_addrs = addrs.iter().map(|(a, p)| (a.to_string(), *p)).collect();
        (NetworkServer { bind_addrs, beast_tx, hex_tx, sbs_tx, incoming_tx: in_tx }, beast_rx, hex_rx, sbs_rx)
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
            let out_rx: broadcast::Receiver<Vec<u8>> = match parser {
                InputParser::Beast => self.beast_tx.subscribe(),
                InputParser::Hex => self.hex_tx.subscribe(),
                InputParser::Sbs => self.sbs_tx.subscribe(),
                InputParser::None => continue,
            };

            tokio::spawn(async move {
                warn!("Client connected: {} ({:?})", addr, parser);
                if let Err(e) = ClientConnection::handle(stream, parser, in_tx, out_rx).await {
                    warn!("Client {} error: {}", addr, e);
                }
            });
        }
    }
}
