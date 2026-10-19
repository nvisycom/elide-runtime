//! Probes over a [`Report`], and the re-encode that closes an
//! anonymize pass.
//!
//! The probes exist because both verbs have to refuse a report
//! that describes nothing, for different reasons: analyze cannot
//! have resolved a modality it has no pipeline for, and anonymize
//! cannot apply an audit that never went through analyze. Both
//! failures otherwise surface as a clean `Ok` over unredacted
//! bytes, which is the one outcome a redaction pipeline must never
//! produce silently.

use bytes::Bytes;
use elide::codec::Document as CodecDocument;
use elide::{Error, ErrorKind, Report, Result};

/// Whether `report` describes any document at all.
///
/// Since elide unified the body into the part tree, a document is a
/// depth-1 part, so this is simply "did anything land". Analyze uses
/// it to reject a document whose codec resolved to a modality with
/// no registered pipeline; anonymize uses it to reject an audit that
/// never went through analyze.
///
/// Deliberately not the per-modality `entities::<M>()` probe it
/// replaced: that shorthand answers only for a *sole* document, so a
/// multi-document report would read as empty and be refused.
pub(super) fn has_documents(report: &Report) -> bool {
    document_names(report).next().is_some()
}

/// The names of the documents `report` describes, one per depth-1
/// part.
pub(super) fn document_names(report: &Report) -> impl Iterator<Item = &str> {
    report
        .part_ids()
        .filter(|(id, _)| id.depth() == 1)
        .map(|(id, _)| id.last_segment())
}

/// Whether `report` describes a document called `name`.
pub(super) fn names_a_document(report: &Report, name: &str) -> bool {
    document_names(report).any(|known| known == name)
}

/// Re-encode the redacted document back into native bytes.
///
/// [`Document::encode`] assembles the parts itself: each stream
/// part re-encodes its own content, each blob contributes its
/// staged bytes, and the format's recombiner joins them. Blob
/// sub-parts are folded in before this runs, so nothing here has
/// to walk the tree or pick a modality — which is what the engine
/// previously needed an untyped handle and a per-modality dispatch
/// for.
///
/// The codec's error is mapped into a redaction-stage one, so a
/// failure here is not mistaken for malformed input.
///
/// [`Document::encode`]: elide::codec::Document::encode
pub(super) fn encode_redacted(document: &CodecDocument) -> Result<Bytes> {
    let content = document.encode().map_err(|err| {
        Error::new(
            ErrorKind::Redaction,
            format!("post-apply encode failed: {err}"),
        )
    })?;
    Ok(content.into_bytes())
}
