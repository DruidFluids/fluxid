//! Local named-pipe IPC: the service publishes one JSON snapshot per
//! connection and the widget polls it.

use anyhow::Result;
use flux_core::sensor_data::SensorSnapshot;
use std::io::{BufRead, BufReader, Write};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, ToNsName};
use interprocess::local_socket::traits::Listener as _;

type IpcListener = interprocess::local_socket::Listener;
type IpcStream = interprocess::local_socket::Stream;

const SOCKET_NAME: &str = "Flux.sock";

pub struct IpcServer {
    listener: IpcListener,
}

impl IpcServer {
    pub fn bind() -> Result<Self> {
        let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
        let opts = ListenerOptions::new().name(name);
        let listener = opts.create_sync()?;
        tracing::info!("IPC server listening on {}", SOCKET_NAME);
        Ok(Self { listener })
    }

    pub fn accept_and_send(&self, snapshot: &SensorSnapshot) -> Result<()> {
        let conn = self.listener.accept()?;
        let json = serde_json::to_string(snapshot)?;
        let mut writer = std::io::BufWriter::new(conn);
        writeln!(writer, "{}", json)?;
        writer.flush()?;
        Ok(())
    }

    pub fn broadcast_loop(
        &self,
        rx: std::sync::mpsc::Receiver<SensorSnapshot>,
    ) -> Result<()> {
        for snapshot in rx {
            if let Err(e) = self.accept_and_send(&snapshot) {
                tracing::trace!("IPC send error (no client connected): {}", e);
            }
        }
        Ok(())
    }
}

pub struct IpcClient;

impl IpcClient {
    pub fn connect() -> Result<SensorSnapshot> {
        use interprocess::local_socket::traits::Stream as _;
        let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
        let conn = IpcStream::connect(name)?;
        let reader = BufReader::new(conn);
        // The service writes exactly one JSON snapshot per connection, so read
        // just the first line.
        if let Some(line) = reader.lines().next() {
            let snapshot: SensorSnapshot = serde_json::from_str(&line?)?;
            return Ok(snapshot);
        }
        anyhow::bail!("No data received from service")
    }

    pub fn poll_loop<F: Fn(SensorSnapshot)>(callback: F, interval_ms: u64) -> Result<()> {
        loop {
            match Self::connect() {
                Ok(snapshot) => callback(snapshot),
                Err(e) => tracing::trace!("IPC poll error: {}", e),
            }
            std::thread::sleep(std::time::Duration::from_millis(interval_ms));
        }
    }
}
