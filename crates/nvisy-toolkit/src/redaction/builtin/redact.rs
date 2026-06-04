//! [`Redact`]: delete the matched span entirely.
//!
//! The codec writes nothing back at the entity's location; the span
//! disappears from the output. This is the strongest text operator —
//! no trace of the original value or its shape remains.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};

use crate::redaction::{Anonymizer, LeakProfile, TextReplacement};

/// Delete the matched span entirely.
#[derive(Debug, Clone, Copy, Default)]
pub struct Redact;

#[async_trait]
impl Anonymizer<Text> for Redact {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Irrecoverable
    }

    async fn apply(&self, _entity: &Entity<Text>, _source: &TextData) -> Result<TextReplacement> {
        Ok(TextReplacement::Removed)
    }
}
