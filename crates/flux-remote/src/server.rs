//! TLS sensor-feed server. Listens on the configured port, authenticates each
//! client with an HMAC challenge, then streams newline-delimited JSON
//! snapshots. Mirrors the C# `Fluid.Service.TcpServer`.

use crate::{protocol, tls};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::RngCore;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_rustls::TlsAcceptor;

const MAX_FAILS: u32 = 5;
const LOCKOUT_SECS: u64 = 60;

type RateMap = Arc<Mutex<HashMap<String, (u32, Instant)>>>;

/// Run the accept loop until the task is aborted. `snapshot_tx` is the
/// broadcast channel the widget pushes local snapshots into.
pub async fn serve(
    port: u16,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    hmac_secret: [u8; 32],
    snapshot_tx: broadcast::Sender<flux_core::sensor_data::SensorSnapshot>,
) -> Result<()> {
    let config = tls::server_config(&cert_der, &key_der)?;
    let acceptor = TlsAcceptor::from(config);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("remote: TCP feed listening on port {port}");

    let rate: RateMap = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("remote: accept error: {e}");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        let acceptor = acceptor.clone();
        let secret = hmac_secret;
        let rx = snapshot_tx.subscribe();
        let rate = rate.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, peer, acceptor, secret, rx, rate).await {
                tracing::debug!("remote: client {peer} ended: {e}");
            }
        });
    }
}

async fn handle_client(
    stream: tokio::net::TcpStream,
    peer: SocketAddr,
    acceptor: TlsAcceptor,
    secret: [u8; 32],
    mut snapshots: broadcast::Receiver<flux_core::sensor_data::SensorSnapshot>,
    rate: RateMap,
) -> Result<()> {
    let ip = peer.ip().to_string();
    if is_locked_out(&rate, &ip).await {
        tracing::warn!("remote: rejected {ip} — rate limited");
        return Ok(());
    }

    let tls = acceptor.accept(stream).await?;
    let (read_half, mut write_half) = tokio::io::split(tls);
    let mut reader = BufReader::new(read_half);

    // Challenge
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    let challenge = serde_json::json!({ "type": "challenge", "nonce": B64.encode(nonce) });
    write_half.write_all(format!("{challenge}\n").as_bytes()).await?;
    write_half.flush().await?;

    // Auth response
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        anyhow::bail!("no auth response");
    }
    let v: serde_json::Value = serde_json::from_str(line.trim())?;
    let provided = v.get("hmac").and_then(|h| h.as_str()).unwrap_or("");
    let provided = B64.decode(provided).unwrap_or_default();
    let expected = protocol::compute_hmac(&nonce, &secret);

    if v.get("type").and_then(|t| t.as_str()) != Some("auth")
        || !protocol::fixed_time_eq(&provided, &expected)
    {
        record_failure(&rate, &ip).await;
        let _ = write_half.write_all(b"{\"type\":\"denied\"}\n").await;
        tracing::warn!("remote: auth failed from {ip}");
        return Ok(());
    }

    reset_failures(&rate, &ip).await;
    write_half.write_all(b"{\"type\":\"ok\"}\n").await?;
    write_half.flush().await?;
    tracing::info!("remote: client authenticated: {ip}");

    // Stream snapshots until the client disconnects or write fails.
    let mut sink = String::new();
    loop {
        tokio::select! {
            recv = snapshots.recv() => {
                match recv {
                    Ok(snap) => {
                        let json = serde_json::to_string(&snap)?;
                        if write_half.write_all(format!("{json}\n").as_bytes()).await.is_err() { break; }
                        if write_half.flush().await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Detect disconnect: a readable EOF returns Ok(0).
            r = reader.read_line(&mut sink) => {
                match r { Ok(0) | Err(_) => break, Ok(_) => { sink.clear(); } }
            }
        }
    }
    tracing::info!("remote: client disconnected: {ip}");
    Ok(())
}

async fn is_locked_out(rate: &RateMap, ip: &str) -> bool {
    let map = rate.lock().await;
    if let Some((fails, until)) = map.get(ip) {
        return *fails >= MAX_FAILS && Instant::now() < *until;
    }
    false
}

async fn record_failure(rate: &RateMap, ip: &str) {
    let mut map = rate.lock().await;
    let entry = map.entry(ip.to_string()).or_insert((0, Instant::now()));
    entry.0 += 1;
    entry.1 = Instant::now() + Duration::from_secs(LOCKOUT_SECS);
}

async fn reset_failures(rate: &RateMap, ip: &str) {
    rate.lock().await.remove(ip);
}
