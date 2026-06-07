//! [`Keep`]: pass the matched span through unchanged.
//!
//! Useful in mixed policies — e.g. mask every kind by default but
//! keep `EntityKind::Currency` so prices remain readable. The
//! replacement records the original value verbatim so the audit
//! trail still has a row.

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};

use crate::redaction::{Anonymizer, LeakProfile, TextReplacement};

/// Pass the matched span through unchanged.
#[derive(Debug, Clone, Copy, Default)]
pub struct Keep;

#[async_trait]
impl Anonymizer<Text> for Keep {
    fn leak_profile(&self) -> LeakProfile {
        // The original value is unchanged — strictly more leaky than
        // every other operator. The `Recoverable` ordering matches
        // "the original is trivially derivable from the output."
        LeakProfile::Recoverable
    }

    async fn apply(&self, _entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let value = source.text.as_str();
        Ok(TextReplacement::substituted(value))
    }
}
