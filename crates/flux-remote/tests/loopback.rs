//! End-to-end loopback test: a server and client authenticate over TLS and a
//! snapshot streams across.

use flux_core::sensor_data::SensorSnapshot;
use flux_remote::{client, identity::ServerIdentity, server, tls};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn loopback_auth_and_stream() {
    tls::ensure_provider();
    let id = ServerIdentity::generate().expect("identity");
    let key = id.handshake_key();
    let port = 55731;

    let (tx, _) = broadcast::channel::<SensorSnapshot>(8);
    {
        let (cert, kd, secret, txc) = (id.cert_der.clone(), id.key_der.clone(), id.hmac_secret, tx.clone());
        tokio::spawn(async move {
            let _ = server::serve(port, cert, kd, secret, txc).await;
        });
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 1. Auth works with the right key.
    assert!(client::test("127.0.0.1", port, &key).await.is_none(), "valid key should authenticate");

    // 2. A bogus key is rejected.
    let bad = "FM1:AAAA";
    assert!(client::test("127.0.0.1", port, bad).await.is_some(), "bad key should fail");

    // 3. Snapshots stream across.
    let (cev_tx, mut cev_rx) = mpsc::unbounded_channel::<client::ClientEvent>();
    tokio::spawn(client::run("127.0.0.1".to_string(), port, key, cev_tx));

    // Keep broadcasting a recognisable snapshot while the client connects.
    let pusher = {
        let tx = tx.clone();
        tokio::spawn(async move {
            for _ in 0..50 {
                let snap = SensorSnapshot { timestamp: 42, ..Default::default() };
                let _ = tx.send(snap);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };

    let got = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            match cev_rx.recv().await {
                Some(client::ClientEvent::Snapshot(s)) => return Some(s),
                Some(_) => continue,
                None => return None,
            }
        }
    })
    .await
    .expect("timed out waiting for snapshot");

    pusher.abort();
    assert_eq!(got.expect("snapshot").timestamp, 42, "snapshot should round-trip");
}
