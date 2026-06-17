//! Remote monitoring for Flux: share this machine's sensors over TLS
//! and consume other machines' feeds.
//!
//! The [`RemoteManager`] owns a background tokio runtime that runs the server
//! and one client per configured device. The UI thread talks to it through a
//! synchronous command API and drains [`RemoteEvent`]s off an `std::mpsc`
//! channel on its normal tick.

pub mod client;
pub mod identity;
pub mod protocol;
pub mod server;
pub mod tls;

pub use identity::ServerIdentity;
pub use protocol::DEFAULT_PORT;

use flux_core::sensor_data::SensorSnapshot;
use flux_core::settings::RemoteDevice;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

/// Events pushed from the background runtime up to the UI.
#[derive(Debug, Clone)]
pub enum RemoteEvent {
    /// The handshake key changed (after Regenerate).
    KeyChanged(String),
    /// A device's connection state changed.
    ConnState { device_id: String, connected: bool },
    /// A fresh snapshot arrived from a device. Boxed (it dwarfs the other
    /// variants) to keep the enum small.
    Snapshot { device_id: String, snapshot: Box<SensorSnapshot> },
    /// Result of a one-shot connection test.
    TestResult { ok: bool, message: String },
}

enum Cmd {
    SetServerEnabled(bool),
    RegenerateKey,
    PushSnapshot(SensorSnapshot),
    SetDevices(Vec<RemoteDevice>),
    TestDevice { host: String, port: u16, key: String },
}

/// Handle to the remote-monitoring background runtime.
pub struct RemoteManager {
    cmd_tx: mpsc::UnboundedSender<Cmd>,
}

impl RemoteManager {
    /// Start the runtime. Returns the manager, the event receiver to drain on
    /// the UI tick, and the current handshake key.
    pub fn start(port: u16) -> (Self, std::sync::mpsc::Receiver<RemoteEvent>, String) {
        let identity_path = ServerIdentity::default_path();
        let identity = ServerIdentity::load_or_create(&identity_path)
            .unwrap_or_else(|_| ServerIdentity::generate().expect("generate identity"));
        let key = identity.handshake_key();

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("flux-remote".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("build remote runtime");
                rt.block_on(manager_loop(identity, identity_path, port, cmd_rx, event_tx));
            })
            .expect("spawn remote thread");

        (Self { cmd_tx }, event_rx, key)
    }

    pub fn set_server_enabled(&self, on: bool) {
        let _ = self.cmd_tx.send(Cmd::SetServerEnabled(on));
    }
    pub fn regenerate_key(&self) {
        let _ = self.cmd_tx.send(Cmd::RegenerateKey);
    }
    pub fn push_snapshot(&self, snap: SensorSnapshot) {
        let _ = self.cmd_tx.send(Cmd::PushSnapshot(snap));
    }
    pub fn set_devices(&self, devices: Vec<RemoteDevice>) {
        let _ = self.cmd_tx.send(Cmd::SetDevices(devices));
    }
    pub fn test_device(&self, host: String, port: u16, key: String) {
        let _ = self.cmd_tx.send(Cmd::TestDevice { host, port, key });
    }
}

struct ClientTask {
    host: String,
    port: u16,
    key: String,
    handles: Vec<JoinHandle<()>>,
}

impl ClientTask {
    fn abort(self) {
        for h in self.handles {
            h.abort();
        }
    }
}

async fn manager_loop(
    mut identity: ServerIdentity,
    identity_path: PathBuf,
    port: u16,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    event_tx: std::sync::mpsc::Sender<RemoteEvent>,
) {
    tls::ensure_provider();

    let (snapshot_tx, _) = broadcast::channel::<SensorSnapshot>(8);
    let mut server_task: Option<JoinHandle<()>> = None;
    let mut clients: HashMap<String, ClientTask> = HashMap::new();

    let start_server = |identity: &ServerIdentity| -> JoinHandle<()> {
        let cert = identity.cert_der.clone();
        let key = identity.key_der.clone();
        let secret = identity.hmac_secret;
        let tx = snapshot_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = server::serve(port, cert, key, secret, tx).await {
                tracing::warn!("remote: server stopped: {e}");
            }
        })
    };

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Cmd::SetServerEnabled(on) => {
                if on {
                    if server_task.is_none() {
                        server_task = Some(start_server(&identity));
                    }
                } else if let Some(h) = server_task.take() {
                    h.abort();
                }
            }
            Cmd::RegenerateKey => {
                if let Ok(new_id) = ServerIdentity::generate() {
                    identity = new_id;
                    let _ = identity.save(&identity_path);
                    let _ = event_tx.send(RemoteEvent::KeyChanged(identity.handshake_key()));
                    if let Some(h) = server_task.take() {
                        h.abort();
                        server_task = Some(start_server(&identity));
                    }
                }
            }
            Cmd::PushSnapshot(snap) => {
                let _ = snapshot_tx.send(snap);
            }
            Cmd::TestDevice { host, port, key } => {
                let tx = event_tx.clone();
                tokio::spawn(async move {
                    let result = client::test(&host, port, &key).await;
                    let _ = tx.send(RemoteEvent::TestResult {
                        ok: result.is_none(),
                        message: result.unwrap_or_else(|| "Connected".into()),
                    });
                });
            }
            Cmd::SetDevices(devices) => {
                reconcile_clients(&mut clients, devices, &event_tx);
            }
        }
    }
}

fn reconcile_clients(
    clients: &mut HashMap<String, ClientTask>,
    devices: Vec<RemoteDevice>,
    event_tx: &std::sync::mpsc::Sender<RemoteEvent>,
) {
    let desired: HashMap<String, RemoteDevice> =
        devices.into_iter().map(|d| (d.id.clone(), d)).collect();

    // Stop clients that are gone or whose config changed.
    let to_stop: Vec<String> = clients
        .iter()
        .filter(|(id, t)| match desired.get(*id) {
            None => true,
            Some(d) => d.host != t.host || d.port != t.port || d.key != t.key,
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in to_stop {
        if let Some(t) = clients.remove(&id) {
            t.abort();
            let _ = event_tx.send(RemoteEvent::ConnState { device_id: id, connected: false });
        }
    }

    // Start clients for newly-added (or changed, now-removed) devices.
    for (id, d) in desired {
        if id.is_empty() || clients.contains_key(&id) {
            continue;
        }
        let (cev_tx, mut cev_rx) = mpsc::unbounded_channel::<client::ClientEvent>();
        let run_handle = tokio::spawn(client::run(d.host.clone(), d.port, d.key.clone(), cev_tx));

        let ev = event_tx.clone();
        let fid = id.clone();
        let fwd_handle = tokio::spawn(async move {
            while let Some(e) = cev_rx.recv().await {
                let msg = match e {
                    client::ClientEvent::State(c) => {
                        RemoteEvent::ConnState { device_id: fid.clone(), connected: c }
                    }
                    client::ClientEvent::Snapshot(s) => {
                        RemoteEvent::Snapshot { device_id: fid.clone(), snapshot: s }
                    }
                };
                if ev.send(msg).is_err() {
                    break;
                }
            }
        });

        clients.insert(
            id,
            ClientTask { host: d.host, port: d.port, key: d.key, handles: vec![run_handle, fwd_handle] },
        );
    }
}
