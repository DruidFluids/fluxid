//! Wire protocol for Flux remote monitoring.
//!
//! Mirrors the C# `Fluid.Shared.Protocol.TcpProtocol`:
//!   * Handshake key format `FM1:<base64(certSHA256[32] || hmacSecret[32])>`.
//!   * HMAC-SHA256 challenge/response for authentication.
//!   * Snapshots are newline-delimited JSON over a TLS channel.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

pub const DEFAULT_PORT: u16 = 5199;
pub const KEY_PREFIX: &str = "FM1:";

/// Reconnect delay after a dropped connection (matches C# ClientReconnectDelayMs).
pub const CLIENT_RECONNECT_DELAY_MS: u64 = 3000;
/// Watchdog: force a reconnect if no snapshot arrives within this window.
pub const WATCHDOG_SECONDS: u64 = 30;

/// Encode a cert fingerprint + HMAC secret into the user-facing handshake key.
pub fn encode_handshake_key(cert_fingerprint: &[u8; 32], hmac_secret: &[u8; 32]) -> String {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(cert_fingerprint);
    combined[32..].copy_from_slice(hmac_secret);
    format!("{}{}", KEY_PREFIX, B64.encode(combined))
}

/// Decode a handshake key back into (cert fingerprint, HMAC secret).
/// Returns `None` if the key is malformed.
pub fn decode_handshake_key(key: &str) -> Option<([u8; 32], [u8; 32])> {
    let body = key.strip_prefix(KEY_PREFIX)?;
    let raw = B64.decode(body.trim()).ok()?;
    if raw.len() != 64 {
        return None;
    }
    let mut fp = [0u8; 32];
    let mut secret = [0u8; 32];
    fp.copy_from_slice(&raw[..32]);
    secret.copy_from_slice(&raw[32..]);
    Some((fp, secret))
}

/// SHA-256 fingerprint of a certificate's raw DER bytes.
pub fn cert_fingerprint(der: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(der);
    h.finalize().into()
}

/// HMAC-SHA256 of a challenge nonce using the shared secret.
pub fn compute_hmac(nonce: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(nonce);
    mac.finalize().into_bytes().to_vec()
}

/// Constant-time comparison of two byte slices.
pub fn fixed_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
