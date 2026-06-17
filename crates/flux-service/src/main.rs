//! Background sensor service: polls hardware on a timer and serves the latest
//! snapshot over the local IPC socket.

use anyhow::Result;
use flux_ipc::IpcServer;
use flux_sensor::SensorPoller;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

const POLL_INTERVAL_MS: u64 = 1000;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("Flux sensor service starting");

    let (tx, rx) = mpsc::channel();

    // Sensor polling thread
    let sensor_tx = tx.clone();
    thread::spawn(move || {
        let mut poller = SensorPoller::new();
        loop {
            let snapshot = poller.poll();
            if sensor_tx.send(snapshot).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        }
    });

    // IPC server -- accepts connections and sends latest snapshot
    let server = IpcServer::bind()?;
    tracing::info!("Service ready, polling every {}ms", POLL_INTERVAL_MS);
    server.broadcast_loop(rx)?;

    Ok(())
}
