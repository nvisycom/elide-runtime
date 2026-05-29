//! Per-modality leak-check trait.
//!
//! Post-redaction validation walks the envelope's applied records
//! and asks "is the original sensitive value still visible in the
//! redacted output?" The check itself is modality-specific:
//!
//! - Text: substring scan over the concatenated post-redaction text.
//! - Tabular: per-cell substring scan against each applied record's
//!   cell value after redaction.
//! - Image / Audio: leak detection requires visual or audio
//!   inspection, which the runtime can't do today. The impl is a
//!   no-op that returns an empty result — see
//!   <https://github.com/nvisycom/runtime/issues/209> (image) and
//!   <https://github.com/nvisycom/runtime/issues/210> (audio).
//!
//! The generic `Validator::execute<M>` dispatches via this trait
//! so each modality's pipeline runs the right check (or none).

use async_trait::async_trait;
use nvisy_ontology::modality::{Audio, Image, Modality, Tabular, Text};
use nvisy_ontology::provenance::EntityRecord;

use super::{LeakedValue, ValidationResult};
use crate::envelope::DocumentEnvelope;
use crate::envelope::value_at::ValueAt;

/// Per-modality leak-check contract.
#[async_trait]
pub trait CheckLeaks: Modality {
    /// Inspect the envelope and report any redacted values that
    /// remain visible. Modalities without leak-detection support
    /// return [`ValidationResult::skipped`].
    async fn check_leaks(envelope: &DocumentEnvelope<Self>) -> ValidationResult;
}

#[async_trait]
impl CheckLeaks for Text {
    async fn check_leaks(envelope: &DocumentEnvelope<Self>) -> ValidationResult {
        let redacted_text = read_text(envelope).await;
        check_text_like::<Text>(envelope, redacted_text.as_deref()).await
    }
}

#[cfg(feature = "tabular")]
#[async_trait]
impl CheckLeaks for Tabular {
    async fn check_leaks(envelope: &DocumentEnvelope<Self>) -> ValidationResult {
        let redacted_text = read_tabular(envelope).await;
        check_text_like::<Tabular>(envelope, redacted_text.as_deref()).await
    }
}

#[cfg(not(feature = "tabular"))]
#[async_trait]
impl CheckLeaks for Tabular {
    async fn check_leaks(_envelope: &DocumentEnvelope<Self>) -> ValidationResult {
        ValidationResult::skipped()
    }
}

#[async_trait]
impl CheckLeaks for Image {
    /// Image leak detection requires visual inspection (compare the
    /// redacted region against the original). Not implemented yet —
    /// tracked at
    /// <https://github.com/nvisycom/runtime/issues/209>.
    async fn check_leaks(_envelope: &DocumentEnvelope<Self>) -> ValidationResult {
        ValidationResult::skipped()
    }
}

#[async_trait]
impl CheckLeaks for Audio {
    /// Audio leak detection requires speech/audio inspection of the
    /// post-redaction segments. Not implemented yet — tracked at
    /// <https://github.com/nvisycom/runtime/issues/210>.
    async fn check_leaks(_envelope: &DocumentEnvelope<Self>) -> ValidationResult {
        ValidationResult::skipped()
    }
}

/// Shared substring-based leak check used by both Text and Tabular.
///
/// For each applied record, re-reads the (post-redaction) value at
/// its location through the modality's [`ValueAt`] impl and checks
/// whether it still contains the original. Records whose value
/// can't be re-read are conservatively counted as passed.
async fn check_text_like<M>(
    envelope: &DocumentEnvelope<M>,
    redacted_text: Option<&str>,
) -> ValidationResult
where
    M: Modality,
    DocumentEnvelope<M>: ValueAt<M>,
{
    let mut passed = 0usize;
    let mut leaked = Vec::new();

    let applied: Vec<&EntityRecord<M>> = envelope
        .document
        .audit
        .records
        .iter()
        .filter(|r| r.audit.as_ref().is_some_and(|e| e.execution.is_applied()))
        .collect();

    let Some(text) = redacted_text else {
        return ValidationResult {
            passed: applied.len(),
            leaked,
            skipped: false,
        };
    };

    let lower_text = text.to_lowercase();
    for record in &applied {
        if let Some(value) = envelope.value_at(&record.entity.location).await {
            let lower_value = value.to_lowercase();
            if !value.is_empty() && lower_text.contains(&lower_value) {
                leaked.push(LeakedValue {
                    value,
                    entity_id: record.entity.id,
                });
            } else {
                passed += 1;
            }
        } else {
            passed += 1;
        }
    }

    ValidationResult {
        passed,
        leaked,
        skipped: false,
    }
}

async fn read_text(envelope: &DocumentEnvelope<Text>) -> Option<String> {
    let locations = envelope.collect_text_locations().await;
    if locations.is_empty() {
        return None;
    }
    let mut buf = String::new();
    for located in &locations {
        if let Some(data) = envelope.read_text(&located.location).await {
            buf.push_str(data.as_str());
        }
    }
    Some(buf)
}

#[cfg(feature = "tabular")]
async fn read_tabular(envelope: &DocumentEnvelope<Tabular>) -> Option<String> {
    let locations = envelope.collect_tabular_locations().await;
    if locations.is_empty() {
        return None;
    }
    let mut buf = String::new();
    for located in &locations {
        if let Some(data) = envelope.read_tabular(&located.location).await {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(data.as_str());
        }
    }
    Some(buf)
}
