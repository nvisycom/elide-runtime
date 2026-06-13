//! [`Hash`]: replace the matched value with a one-way SHA-2 hash.
//!
//! Configurable along two axes:
//!
//! - `algorithm` — [`HashAlgorithm::Sha256`] (default) or
//!   [`HashAlgorithm::Sha512`].
//! - `salt` — optional bytes prepended to the value before hashing.
//!   Salting blocks pre-computed rainbow attacks and makes equal
//!   plaintext values hash to different ciphertext across deployments.
//!
//! The output is the lowercase hex representation of the digest.

use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};
use sha2::{Digest, Sha256, Sha512};

use crate::redaction::{Anonymizer, LeakProfile, TextReplacement};

/// Which SHA-2 variant to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    /// SHA-256 — 32-byte digest, 64-char hex.
    #[default]
    Sha256,
    /// SHA-512 — 64-byte digest, 128-char hex.
    Sha512,
}

/// One-way SHA-2 hash operator.
#[derive(Debug, Clone, Default)]
pub struct Hash {
    algorithm: HashAlgorithm,
    salt: Vec<u8>,
}

impl Hash {
    /// Build a hash operator with the given algorithm and no salt.
    pub fn new(algorithm: HashAlgorithm) -> Self {
        Self {
            algorithm,
            salt: Vec::new(),
        }
    }

    /// SHA-256 with no salt — the most common default.
    pub fn sha256() -> Self {
        Self::new(HashAlgorithm::Sha256)
    }

    /// SHA-512 with no salt.
    pub fn sha512() -> Self {
        Self::new(HashAlgorithm::Sha512)
    }

    /// Attach a salt that's prepended to the value before hashing.
    #[must_use]
    pub fn with_salt(mut self, salt: impl Into<Vec<u8>>) -> Self {
        self.salt = salt.into();
        self
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Hash {
    fn leak_profile(&self) -> LeakProfile {
        // A hash is recoverable when the candidate plaintext space
        // is small enough to brute-force or rainbow-table; salting
        // raises the bar but doesn't remove recoverability in the
        // information-theoretic sense.
        LeakProfile::Recoverable
    }

    async fn apply(&self, _entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let digest = match self.algorithm {
            HashAlgorithm::Sha256 => hex(Sha256::new()
                .chain_update(&self.salt)
                .chain_update(source.text.as_str())
                .finalize()
                .as_slice()),
            HashAlgorithm::Sha512 => hex(Sha512::new()
                .chain_update(&self.salt)
                .chain_update(source.text.as_str())
                .finalize()
                .as_slice()),
        };
        Ok(TextReplacement::substituted(digest))
    }
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{TrailStep, builtins};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn sha256_matches_known_vector() {
        // SHA-256("alice") = "2bd806c97f0e00af1a1fc3328fa763a9269723c8db8fac4f93af71db186d6e90"
        let op = Hash::sha256();
        let source = TextData::new("alice");
        let entity = entity(0, 5);
        let out = op.apply(&entity, &source).await.unwrap();
        assert_eq!(
            out,
            TextReplacement::substituted(
                "2bd806c97f0e00af1a1fc3328fa763a9269723c8db8fac4f93af71db186d6e90"
            )
        );
    }

    #[tokio::test]
    async fn salt_changes_digest() {
        let plain = Hash::sha256();
        let salted = Hash::sha256().with_salt("pepper");
        let source = TextData::new("alice");
        let entity = entity(0, 5);
        let a = plain.apply(&entity, &source).await.unwrap();
        let b = salted.apply(&entity, &source).await.unwrap();
        assert_ne!(a, b, "salt must change the digest");
    }

    #[tokio::test]
    async fn sha512_emits_128_hex_chars() {
        let op = Hash::sha512();
        let source = TextData::new("alice");
        let entity = entity(0, 5);
        let TextReplacement::Substituted { value } = op.apply(&entity, &source).await.unwrap()
        else {
            panic!("Hash always substitutes");
        };
        assert_eq!(value.len(), 128);
    }
}
