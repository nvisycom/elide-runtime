//! Substring-based leak detection: post-redaction re-scan.
//!
//! Public surface:
//!
//! - [`CheckLeaks`] — domain-specific trait. Callers who want just
//!   leak detection (skipping the abstract [`Check`] machinery) hold
//!   `&dyn CheckLeaks<M, P>` and call `check_leaks` directly,
//!   getting back typed [`LeakFinding`]s.
//! - [`LeakCheck`] — the canonical implementation. Implements both
//!   [`CheckLeaks<M, P>`] (for direct domain callers) and
//!   [`Check<M, P>`] (so it slots into a [`CheckPipeline`]). The
//!   `Check` impl stamps `self.severity` on every emitted leak.
//! - [`LeakFinding`] — typed output of [`CheckLeaks::check_leaks`].
//!
//! Modality coverage today: [`Text`] and (with the `tabular`
//! feature) [`Tabular`]. Image and Audio leak detection requires
//! visual / audio inspection that the runtime can't do — see
//! <https://github.com/nvisycom/runtime/issues/209> (image) and
//! <https://github.com/nvisycom/runtime/issues/210> (audio).
//!
//! [`Check`]: super::Check
//! [`CheckPipeline`]: super::CheckPipeline

use nvisy_core::ValueAt;
use nvisy_core::modality::{Tabular, Text};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use super::check::{Check, CheckContext, Finding, FindingKind, Severity};
use crate::document::Document;
use crate::modality::DocumentModality;
use crate::provenance::EntityRecord;

/// A single leak detected by [`LeakCheck`].
///
/// Domain-typed output of [`CheckLeaks::check_leaks`]. The bridge
/// `impl Check for LeakCheck` wraps each [`LeakFinding`] into a
/// generic [`Finding`] carrying the configured severity.
#[derive(Debug, Clone)]
pub struct LeakFinding {
    /// The entity whose redacted value remained visible.
    pub entity_id: Uuid,
    /// The original sensitive value found in the redacted output.
    pub value: String,
}

/// Domain-specific trait for leak detection.
///
/// Implementors return a typed [`LeakFinding`] list (one per
/// surviving sensitive value). The abstract [`Check`] impl on the
/// same type translates these into [`Finding`]s with the
/// implementor's configured [`Severity`].
///
/// Today only [`LeakCheck`] implements this; the trait exists so
/// custom leak strategies (LLM-based scan, n-gram check, …) can plug
/// in without touching the abstract `Check` glue.
///
/// [`Check`]: super::Check
#[async_trait::async_trait]
pub trait CheckLeaks<M, P>: Send + Sync
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
{
    /// Inspect `doc` and emit a typed list of leak findings.
    async fn check_leaks(
        &self,
        doc: &Document<M>,
        ctx: &CheckContext<'_, M, P>,
    ) -> Vec<LeakFinding>;
}

/// Canonical [`CheckLeaks`] implementation.
///
/// Substring-scans the post-redaction codec output for each applied
/// entity's original value. Folds via NFC + lowercase before
/// matching to be resilient to Unicode normalization and ASCII case
/// differences.
pub struct LeakCheck {
    severity: Severity,
}

impl LeakCheck {
    /// Build a leak check whose findings are stamped with `severity`.
    pub fn new(severity: Severity) -> Self {
        Self { severity }
    }
}

#[async_trait::async_trait]
impl<P> CheckLeaks<Text, P> for LeakCheck
where
    P: ValueAt<Text> + ?Sized,
{
    async fn check_leaks(
        &self,
        doc: &Document<Text>,
        ctx: &CheckContext<'_, Text, P>,
    ) -> Vec<LeakFinding> {
        check_text_like::<Text, P>(doc, ctx.resolver, ctx.redacted_output).await
    }
}

#[async_trait::async_trait]
impl<P> CheckLeaks<Tabular, P> for LeakCheck
where
    P: ValueAt<Tabular> + ?Sized,
{
    async fn check_leaks(
        &self,
        doc: &Document<Tabular>,
        ctx: &CheckContext<'_, Tabular, P>,
    ) -> Vec<LeakFinding> {
        check_text_like::<Tabular, P>(doc, ctx.resolver, ctx.redacted_output).await
    }
}

// Bridge: every modality where LeakCheck impls CheckLeaks also gets
// an abstract Check impl that stamps the configured severity onto
// each leak finding.
#[async_trait::async_trait]
impl<M, P> Check<M, P> for LeakCheck
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
    LeakCheck: CheckLeaks<M, P>,
{
    async fn check(&self, doc: &Document<M>, ctx: &CheckContext<'_, M, P>) -> Vec<Finding> {
        let leaks = <Self as CheckLeaks<M, P>>::check_leaks(self, doc, ctx).await;
        leaks
            .into_iter()
            .map(|l| {
                let message = format!(
                    "redacted value {:?} for entity {} still present in output",
                    l.value, l.entity_id
                );
                Finding {
                    severity: self.severity,
                    kind: FindingKind::Leak {
                        entity_id: l.entity_id,
                        value: l.value,
                    },
                    message,
                }
            })
            .collect()
    }
}

/// Shared substring-based leak check used by both Text and Tabular.
///
/// For each applied record, re-reads the (post-redaction) value at
/// its location through the modality's [`ValueAt`] impl and checks
/// whether it still contains the original. Records whose value can't
/// be re-read are conservatively counted as passed.
async fn check_text_like<M, P>(
    doc: &Document<M>,
    resolver: &P,
    redacted_text: Option<&str>,
) -> Vec<LeakFinding>
where
    M: DocumentModality,
    P: ValueAt<M> + ?Sized,
{
    let applied: Vec<&EntityRecord<M>> = doc
        .audit
        .records
        .iter()
        .filter(|r| r.audit.as_ref().is_some_and(|e| e.execution.is_applied()))
        .collect();

    let Some(text) = redacted_text else {
        return Vec::new();
    };

    let mut leaks = Vec::new();
    let folded_text = fold_for_match(text);
    for record in &applied {
        if let Some(value) = resolver.value_at(&record.entity.location).await {
            let folded_value = fold_for_match(&value);
            if !value.is_empty() && folded_text.contains(&folded_value) {
                leaks.push(LeakFinding {
                    entity_id: record.entity.id,
                    value,
                });
            }
        }
    }

    leaks
}

/// Fold a string for case-insensitive substring matching across
/// Unicode normalization forms.
///
/// Without normalization, the same character sequence written in NFC
/// vs NFD won't substring-match: e.g. `"café"` as
/// `[c, a, f, U+00E9]` vs `[c, a, f, e, U+0301]` are distinct byte
/// sequences. We normalize to NFC, then lowercase. Turkish dotless-i
/// and other locale-sensitive cases still misfold (`I` → `i` not
/// `ı`), but that's a deeper Unicode rabbit hole and the redaction
/// pipeline doesn't currently carry per-document locale info to do
/// it properly.
fn fold_for_match(s: &str) -> String {
    s.nfc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::fold_for_match;

    #[test]
    fn nfc_normalization_collapses_combining_accents() {
        // "café" as NFC (U+00E9) and NFD (e + U+0301) should fold
        // to the same string.
        let nfc = "caf\u{00e9}";
        let nfd = "cafe\u{0301}";
        assert_eq!(fold_for_match(nfc), fold_for_match(nfd));
    }

    #[test]
    fn case_fold_substring_match_works_across_normalization() {
        // The haystack is NFD and uppercase; the needle is NFC
        // lowercase. Folding both should let the substring match.
        let haystack = fold_for_match("HELLO CAFE\u{0301}!");
        let needle = fold_for_match("caf\u{00e9}");
        assert!(haystack.contains(&needle));
    }
}
