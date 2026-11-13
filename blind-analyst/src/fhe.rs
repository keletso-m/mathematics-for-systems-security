use anyhow::{anyhow, Result};
use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
use std::collections::BTreeMap;

const NONCE_LEN: usize = 12;

/// Output layout: nonce (12) || ciphertext+tag
pub fn encrypt(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| anyhow!("bad key length"))?;
    let mut n = [0u8; NONCE_LEN];
    getrandom::fill(&mut n).map_err(|e| anyhow!("rng failure: {e}"))?;
    let ct = cipher
        .encrypt(&Nonce::from(n), plaintext)
        .map_err(|_| anyhow!("encryption failed"))?;
    let mut out = n.to_vec();
    out.extend(ct);
    Ok(out)
}

pub fn decrypt(key: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_LEN {
        return Err(anyhow!("blob too short"));
    }
    let cipher = ChaCha20Poly1305::new_from_slice(key).map_err(|_| anyhow!("bad key length"))?;
    let mut n = [0u8; NONCE_LEN];
    n.copy_from_slice(&blob[..NONCE_LEN]);
    cipher
        .decrypt(&Nonce::from(n), &blob[NONCE_LEN..])
        .map_err(|_| anyhow!("decryption failed (wrong key or tampered data)"))
}

/// Maps opaque tokens like {{SECRET_1}} to ciphertext.
/// Holds NO key; the key is passed in only when sealing or revealing.
#[derive(Default)]
pub struct TokenStore {
    map: BTreeMap<String, Vec<u8>>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace each secret in `text` with a token; store its ciphertext.
    pub fn redact(&mut self, key: &[u8], text: &str, secrets: &[&str]) -> Result<String> {
        let mut out = text.to_string();
        for s in secrets {
            let token = format!("{{{{SECRET_{}}}}}", self.map.len() + 1);
            self.map.insert(token.clone(), encrypt(key, s.as_bytes())?);
            out = out.replace(s, &token);
        }
        Ok(out)
    }

    /// Replace tokens in `text` with decrypted plaintext (local use only!).
    pub fn reveal(&self, key: &[u8], text: &str) -> Result<String> {
        let mut out = text.to_string();
        for (token, blob) in &self.map {
            if out.contains(token) {
                let pt = String::from_utf8(decrypt(key, blob)?)?;
                out = out.replace(token, &pt);
            }
        }
        Ok(out)
    }

    /// What an observer on the host provider side could see.
    pub fn ciphertext_hex(&self) -> Vec<(String, String)> {
        self.map
            .iter()
            .map(|(t, b)| (t.clone(), b.iter().map(|x| format!("{x:02x}")).collect()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper() {
        let key = [7u8; 32];
        let mut blob = encrypt(&key, b"hello").unwrap();
        assert_eq!(decrypt(&key, &blob).unwrap(), b"hello");
        *blob.last_mut().unwrap() ^= 1;
        assert!(decrypt(&key, &blob).is_err());
    }

    #[test]
    fn redact_then_reveal() {
        let key = [9u8; 32];
        let mut store = TokenStore::new();
        let red = store
            .redact(&key, "Pay Alice 4111-1111 now", &["Alice", "4111-1111"])
            .unwrap();
        assert!(!red.contains("Alice") && !red.contains("4111"));
        assert_eq!(store.reveal(&key, &red).unwrap(), "Pay Alice 4111-1111 now");
    }
}