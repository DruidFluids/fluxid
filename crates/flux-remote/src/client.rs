//! TLS client that connects to a remote Flux feed, authenticates with
//! the HMAC challenge, and streams snapshots. Auto-reconnects on drop with a
//! watchdog. Mirrors the C# `RemoteTcpClient`.

use crate::{protocol, tls};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use flux_core::sensor_data::SensorSnapshot;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// Events emitted by a running client loop. The snapshot is boxed because it
/// dwarfs the other variant (≈240 B vs 1 B).
pub enum ClientEvent {
    State(bool),
    Snapshot(Box<SensorSnapshot>),
}

/// Connect + authenticate, returning a split reader/writer on success.
async fn connect_and_auth(
    host: &str,
    port: u16,
    key: &str,
) -> Result<
    (
        BufReader<tokio::io::ReadHalf<tokio_rustls::client::TlsStream<TcpStream>>>,
        tokio::io::WriteHalf<tokio_rustls::client::TlsStream<TcpStream>>,
    ),
    String,
> {
    let (fp, secret) = protocol::decode_handshake_key(key)
        .ok_or_else(|| "Invalid Handshake Key format".to_string())?;

    let connector = TlsConnector::from(tls::client_config(fp));
    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| e.to_string())?;
    let tls = connector
        .connect(tls::server_name(), tcp)
        .await
        .map_err(|e| e.to_string())?;

    let (read_half, mut write_half) = tokio::io::split(tls);
    let mut reader = BufReader::new(read_half);

    // Read challenge
    let mut line = String::new();
    if reader.read_line(&mut line).await.map_err(|e| e.to_string())? == 0 {
        return Err("No challenge received".into());
    }
    let v: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;
    let nonce_b64 = v.get("nonce").and_then(|n| n.as_str()).unwrap_or("");
    let nonce = B64.decode(nonce_b64).map_err(|e| e.to_string())?;

    // Send auth
    let hmac = protocol::compute_hmac(&nonce, &secret);
    let auth = serde_json::json!({ "type": "auth", "hmac": B64.encode(hmac) });
    write_half
        .write_all(format!("{auth}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    write_half.flush().await.map_err(|e| e.to_string())?;

    // Read ok/denied
    line.clear();
    if reader.read_line(&mut line).await.map_err(|e| e.to_string())? == 0 {
        return Err("No auth response".into());
    }
    let v: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;
    if v.get("type").and_then(|t| t.as_str()) != Some("ok") {
        return Err("Authentication denied — verify the Handshake Key".into());
    }

    Ok((reader, write_half))
}

/// One-shot connection test. Returns `None` on success, or an error message.
pub async fn test(host: &str, port: u16, key: &str) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(8), connect_and_auth(host, port, key)).await {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e),
        Err(_) => Some("Connection timed out".into()),
    }
}

/// Run the persistent client loop: connect, stream, reconnect on drop.
/// Emits state changes and snapshots through `tx`. Runs until aborted.
pub async fn run(host: String, port: u16, key: String, tx: tokio::sync::mpsc::UnboundedSender<ClientEvent>) {
    loop {
        match connect_and_auth(&host, port, &key).await {
            Ok((mut reader, _writer)) => {
                let _ = tx.send(ClientEvent::State(true));
                let mut line = String::new();
                loop {
                    line.clear();
                    let read = tokio::time::timeout(
                        Duration::from_secs(protocol::WATCHDOG_SECONDS),
                        reader.read_line(&mut line),
                    )
                    .await;
                    match read {
                        Ok(Ok(0)) | Err(_) => break, // EOF or watchdog timeout
                        Ok(Err(_)) => break,
                        Ok(Ok(_)) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if let Ok(snap) = serde_json::from_str::<SensorSnapshot>(trimmed) {
                                let _ = tx.send(ClientEvent::Snapshot(Box::new(snap)));
                            }
                        }
                    }
                }
                let _ = tx.send(ClientEvent::State(false));
            }
            Err(_) => {
                let _ = tx.send(ClientEvent::State(false));
            }
        }
        tokio::time::sleep(Duration::from_millis(protocol::CLIENT_RECONNECT_DELAY_MS)).await;
    }
}
