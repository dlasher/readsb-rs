use std::io;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use tracing::{info, warn};
use super::server::{DecodedMessage, InputParser};
use super::protocols::{sbs, hex, beast};

pub struct ClientConnection;

impl ClientConnection {
    pub async fn handle(
        mut stream: TcpStream,
        parser: InputParser,
        incoming_tx: broadcast::Sender<DecodedMessage>,
        mut outgoing_rx: broadcast::Receiver<Vec<u8>>,
    ) -> io::Result<()> {
        let (mut rx, mut tx) = stream.split();

        tokio::select! {
            result = read_loop(&mut rx, parser, incoming_tx) => result,
            result = write_loop(&mut tx, &mut outgoing_rx) => result,
        }
    }
}

async fn read_loop(
    rx: &mut tokio::net::tcp::ReadHalf<'_>,
    parser: InputParser,
    incoming_tx: broadcast::Sender<DecodedMessage>,
) -> io::Result<()> {
    let mut buf = vec![0u8; 65536];
    loop {
        let n = rx.read(&mut buf).await?;
        if n == 0 { break; }

        match parser {
            InputParser::Beast => {
                let mut i = 0;
                while i < n {
                    if let Some(frame) = beast::parse_beast_frame(&buf[i..]) {
                        let payload_len = frame.payload.len();
                        let _ = incoming_tx.send(DecodedMessage { data: frame.payload, client_id: 0 });
                        // Advance past the frame: 9 header bytes + stuffed payload (worst-case 2x)
                        i += 9 + payload_len * 2;
                    } else {
                        i += 1;
                    }
                }
            }
            InputParser::Hex => {
                for line in buf[..n].split(|&b| b == b'\n') {
                    if let Some(bytes) = hex::parse_line(std::str::from_utf8(line).unwrap_or("")) {
                        let _ = incoming_tx.send(DecodedMessage { data: bytes, client_id: 0 });
                    }
                }
            }
            InputParser::Sbs => {
                for line in buf[..n].split(|&b| b == b'\n') {
                    if let Some(sbs) = sbs::parse_line(std::str::from_utf8(line).unwrap_or("")) {
                        let _ = incoming_tx.send(DecodedMessage { data: sbs.hex_ident.as_bytes().to_vec(), client_id: 0 });
                    }
                }
            }
            InputParser::None => {}
        }
    }
    info!("Client disconnected (read)");
    Ok(())
}

async fn write_loop(tx: &mut tokio::net::tcp::WriteHalf<'_>, rx: &mut broadcast::Receiver<Vec<u8>>) -> io::Result<()> {
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if let Err(e) = tx.write_all(&msg).await {
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
