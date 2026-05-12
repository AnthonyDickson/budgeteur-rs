//! API key management for TUI client authentication via Ed25519-signed JWTs.
//!
//! The server loads a list of allowed Ed25519 public keys from a TOML config
//! file. Each request from the TUI carries a JWT signed with the corresponding
//! private key. The server tries each known key until one validates the token.

use ed25519_dalek::VerifyingKey;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;
use std::path::Path;

pub use budgeteur_shared::auth::TuiClaims;

// ---------------------------------------------------------------------------
// Config file format
// ---------------------------------------------------------------------------

/// Top-level structure of the `tui_public_keys.toml` config file.
#[derive(Debug, Deserialize)]
pub struct TuiKeysConfig {
    pub keys: Vec<TuiKeyEntry>,
}

/// A single allowed TUI key entry in the config file.
#[derive(Debug, Deserialize)]
pub struct TuiKeyEntry {
    /// Human-readable label for this key (e.g. "laptop", "desktop").
    pub label: String,
    /// Hex-encoded 32-byte Ed25519 public key.
    pub public_key: String,
}

// ---------------------------------------------------------------------------
// Key store
// ---------------------------------------------------------------------------

/// A collection of allowed Ed25519 public keys, loaded from config at startup.
///
/// Keys are static for the lifetime of the server process.
#[derive(Debug, Clone)]
pub struct TuiKeyStore {
    keys: Vec<VerifyingKey>,
}

/// Errors that can occur while loading or validating TUI keys.
#[derive(Debug, thiserror::Error)]
pub enum TuiKeyError {
    #[error("could not read key config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("could not parse key config TOML: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("invalid hex-encoded public key for '{label}': {source}")]
    InvalidHexKey {
        label: String,
        source: hex::FromHexError,
    },
    #[error("invalid Ed25519 public key for '{label}': {source}")]
    InvalidEd25519Key {
        label: String,
        source: ed25519_dalek::SignatureError,
    },
    #[error("JWT validation failed: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),
}

impl TuiKeyStore {
    /// Create an empty key store (TUI auth is effectively disabled).
    pub fn empty() -> Self {
        Self { keys: Vec::new() }
    }

    /// Load allowed public keys from a TOML file on disk.
    pub fn load(path: &Path) -> Result<Self, TuiKeyError> {
        let contents = std::fs::read_to_string(path)?;
        let config: TuiKeysConfig = toml::from_str(&contents)?;
        Self::load_from_config(&config)
    }

    /// Load keys from an already-parsed config. Useful for testing.
    pub fn load_from_config(config: &TuiKeysConfig) -> Result<Self, TuiKeyError> {
        let mut keys = Vec::with_capacity(config.keys.len());
        for entry in &config.keys {
            let raw =
                hex::decode(&entry.public_key).map_err(|source| TuiKeyError::InvalidHexKey {
                    label: entry.label.clone(),
                    source,
                })?;

            let raw: [u8; 32] = raw.try_into().map_err(|_| TuiKeyError::InvalidHexKey {
                label: entry.label.clone(),
                source: hex::FromHexError::InvalidStringLength,
            })?;

            let key = VerifyingKey::from_bytes(&raw).map_err(|source| {
                TuiKeyError::InvalidEd25519Key {
                    label: entry.label.clone(),
                    source,
                }
            })?;

            keys.push(key);
        }

        Ok(Self { keys })
    }

    /// Whether any keys have been configured. When `false`, all TUI API
    /// requests will be rejected.
    pub fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Validate a JWT bearer token against all stored public keys.
    ///
    /// Returns the decoded claims if any key validates the token.
    /// Returns `None` if no keys are configured or no key validates.
    pub fn validate(&self, token: &str) -> Option<TuiClaims> {
        if self.keys.is_empty() {
            return None;
        }

        let mut validation = Validation::new(Algorithm::EdDSA);
        // We validate `sub` ourselves since we don't care about the exact issuer.
        validation.validate_exp = true;
        validation.required_spec_claims.clear();

        for key in &self.keys {
            let decoding_key = DecodingKey::from_ed_der(&key.to_bytes());

            if let Ok(data) = decode::<TuiClaims>(token, &decoding_key, &validation) {
                return Some(data.claims);
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{SigningKey, pkcs8::EncodePrivateKey};
    use jsonwebtoken::{EncodingKey, Header, encode};
    use rand::Rng;
    use std::io::Write;

    /// Generate a fresh Ed25519 keypair for testing.
    fn generate_keypair() -> (SigningKey, ed25519_dalek::VerifyingKey) {
        let mut seed = [0u8; 32];
        rand::rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        (signing_key, verifying_key)
    }

    /// Create a signed JWT for testing.
    fn sign_jwt(signing_key: &SigningKey, claims: &TuiClaims) -> String {
        let der = signing_key.to_pkcs8_der().unwrap();
        let encoding_key = EncodingKey::from_ed_der(der.as_bytes());

        encode(&Header::new(Algorithm::EdDSA), claims, &encoding_key).unwrap()
    }

    /// Create a TOML config string from a verifying key.
    fn toml_config(label: &str, verifying_key: &VerifyingKey) -> String {
        let hex_key = hex::encode(verifying_key.to_bytes());
        format!("[[keys]]\nlabel = \"{label}\"\npublic_key = \"{hex_key}\"\n")
    }

    /// Write a TOML config to a temp file and return the path.
    fn write_temp_config(contents: &str) -> std::path::PathBuf {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.into_temp_path().keep().unwrap()
    }

    #[test]
    fn load_keys_from_toml_file() {
        // Given a TOML config file with one valid public key
        let (_signing_key, verifying_key) = generate_keypair();
        let config = toml_config("test-key", &verifying_key);
        let path = write_temp_config(&config);

        // When the key store loads that file
        let store = TuiKeyStore::load(&path).unwrap();

        // Then the store has one key and reports having keys
        assert!(store.has_keys());
        assert_eq!(store.keys.len(), 1);
    }

    #[test]
    fn load_multiple_keys() {
        // Given a TOML config file with two valid public keys
        let (_sk1, vk1) = generate_keypair();
        let (_sk2, vk2) = generate_keypair();
        let config = format!(
            "{}\n{}",
            toml_config("key1", &vk1),
            toml_config("key2", &vk2)
        );
        let path = write_temp_config(&config);

        // When the key store loads that file
        let store = TuiKeyStore::load(&path).unwrap();

        // Then the store contains both keys
        assert_eq!(store.keys.len(), 2);
    }

    #[test]
    fn empty_store_has_no_keys() {
        // Given an empty key store
        let store = TuiKeyStore::empty();

        // Then it reports no keys and rejects any token
        assert!(!store.has_keys());
        assert!(store.validate("anything").is_none());
    }

    #[test]
    fn roundtrip_sign_and_verify() {
        // Given an Ed25519 keypair
        let (signing_key, verifying_key) = generate_keypair();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };

        // When a JWT is signed with the private key
        let priv_der = signing_key.to_pkcs8_der().unwrap();
        let encoding_key = EncodingKey::from_ed_der(priv_der.as_bytes());
        let token = encode(&Header::new(Algorithm::EdDSA), &claims, &encoding_key).unwrap();

        // Then the JWT can be verified with the corresponding public key
        let pub_raw = verifying_key.to_bytes();
        let decoding_key = DecodingKey::from_ed_der(&pub_raw);

        let mut validation = Validation::new(Algorithm::EdDSA);
        validation.required_spec_claims.clear();

        let result = decode::<TuiClaims>(&token, &decoding_key, &validation);
        assert!(result.is_ok(), "Roundtrip sign+verify failed: {result:?}");
    }

    #[test]
    fn validate_valid_jwt() {
        // Given a key store loaded with a known public key
        let (signing_key, verifying_key) = generate_keypair();
        let config = toml_config("test-key", &verifying_key);
        let path = write_temp_config(&config);
        let store = TuiKeyStore::load(&path).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When the token is validated
        let result = store.validate(&token);

        // Then the claims are returned
        assert!(result.is_some());
    }

    #[test]
    fn validate_expired_jwt() {
        // Given a key store loaded with a known public key
        let (signing_key, verifying_key) = generate_keypair();
        let config = toml_config("test-key", &verifying_key);
        let path = write_temp_config(&config);
        let store = TuiKeyStore::load(&path).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now - 7200,
            exp: now - 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When the expired token is validated
        let result = store.validate(&token);

        // Then validation fails
        assert!(result.is_none());
    }

    #[test]
    fn validate_jwt_with_wrong_key() {
        // Given a key store loaded with one public key
        let (signing_key, _verifying_key) = generate_keypair();
        let (_other_sk, other_vk) = generate_keypair();
        let config = toml_config("other-key", &other_vk);
        let path = write_temp_config(&config);
        let store = TuiKeyStore::load(&path).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When a token signed with an unknown key is validated
        let result = store.validate(&token);

        // Then validation fails
        assert!(result.is_none());
    }

    #[test]
    fn validate_with_multiple_keys_finds_match() {
        // Given a key store loaded with three keys, where the middle one is correct
        let (signing_key, verifying_key) = generate_keypair();
        let (_sk2, vk2) = generate_keypair();
        let (_sk3, vk3) = generate_keypair();

        let config = format!(
            "{}\n{}\n{}",
            toml_config("wrong1", &vk2),
            toml_config("correct", &verifying_key),
            toml_config("wrong2", &vk3),
        );
        let path = write_temp_config(&config);
        let store = TuiKeyStore::load(&path).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = TuiClaims {
            sub: "tui-client".into(),
            iat: now,
            exp: now + 3600,
        };
        let token = sign_jwt(&signing_key, &claims);

        // When the token is validated
        let result = store.validate(&token);

        // Then validation succeeds by matching the correct key
        assert!(result.is_some());
    }

    #[test]
    fn load_invalid_hex_key_returns_error() {
        // Given a TOML config file with a non-hex public key
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"[[keys]]\nlabel = \"bad\"\npublic_key = \"not-a-valid-hex-key\"\n")
            .unwrap();
        let path = file.into_temp_path().keep().unwrap();

        // When the key store tries to load it
        let result = TuiKeyStore::load(&path);

        // Then loading fails with an error
        assert!(result.is_err());
    }
}
