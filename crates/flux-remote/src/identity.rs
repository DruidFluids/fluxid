//! This machine's stable server identity: a self-signed certificate plus an
//! HMAC secret. Persisted to disk so the handshake key is stable across
//! restarts (regenerating is an explicit user action).

use crate::protocol;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
struct StoredIdentity {
    cert_der_b64: String,
    key_der_b64: String,
    hmac_secret_b64: String,
}

/// A self-signed cert + private key + HMAC secret used to serve the TCP feed.
#[derive(Clone)]
pub struct ServerIdentity {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
    pub hmac_secret: [u8; 32],
}

impl ServerIdentity {
    /// Generate a fresh self-signed cert + random HMAC secret.
    pub fn generate() -> Result<Self> {
        let cert = rcgen::generate_simple_self_signed(vec!["Flux".to_string()])
            .context("generating self-signed certificate")?;
        let cert_der = cert.cert.der().to_vec();
        let key_der = cert.key_pair.serialize_der();
        let mut hmac_secret = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut hmac_secret);
        Ok(Self { cert_der, key_der, hmac_secret })
    }

    /// Load the identity from disk, generating + saving a fresh one if absent
    /// or corrupt.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(stored) = serde_json::from_str::<StoredIdentity>(&text) {
                if let Some(id) = Self::from_stored(&stored) {
                    return Ok(id);
                }
            }
        }
        let id = Self::generate()?;
        id.save(path)?;
        Ok(id)
    }

    fn from_stored(s: &StoredIdentity) -> Option<Self> {
        let cert_der = B64.decode(&s.cert_der_b64).ok()?;
        let key_der = B64.decode(&s.key_der_b64).ok()?;
        let secret = B64.decode(&s.hmac_secret_b64).ok()?;
        if secret.len() != 32 {
            return None;
        }
        let mut hmac_secret = [0u8; 32];
        hmac_secret.copy_from_slice(&secret);
        Some(Self { cert_der, key_der, hmac_secret })
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let stored = StoredIdentity {
            cert_der_b64: B64.encode(&self.cert_der),
            key_der_b64: B64.encode(&self.key_der),
            hmac_secret_b64: B64.encode(self.hmac_secret),
        };
        std::fs::write(path, serde_json::to_string_pretty(&stored)?)?;
        Ok(())
    }

    /// The user-facing handshake key others use to connect to this machine.
    pub fn handshake_key(&self) -> String {
        let fp = protocol::cert_fingerprint(&self.cert_der);
        protocol::encode_handshake_key(&fp, &self.hmac_secret)
    }

    pub fn default_path() -> PathBuf {
        flux_core::settings::AppSettings::config_dir().join("remote_identity.json")
    }
}
