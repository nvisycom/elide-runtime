//! Redaction: [`Engine::anonymize`], the second half of the pass.
//!
//! Where analyze reports, this applies. Most of what the function
//! does before it reaches the orchestrator is refusal: three
//! separate ways an audit can fail to describe the document in
//! hand, each of which would otherwise return the original bytes
//! with a clean `Ok`.

use std::mem;

use elide::{Error, ErrorKind, Report, Result};
use elide_provider::KeyConfig;
use elide_schema::policy::Policy;

use super::Engine;
use super::report::{document_names, encode_redacted, has_documents, names_a_document};
use crate::audit::Audit;
use crate::file::Document;

impl Engine {
    /// Apply `policies` and the audit's reviewer decisions to
    /// `document`, returning the re-encoded result.
    ///
    /// `audit` is taken by `&mut`: each entity gains a redaction
    /// event per operator that fired, so its provenance records who
    /// redacted what, under which rule.
    ///
    /// The label catalog is re-derived on every call, from two
    /// sources. `policies` bring the scopes and the rules, so
    /// governance stays live between the two passes: a policy
    /// edited in the gap takes effect here. The audit brings the
    /// labels the *request* introduced, because a caller
    /// re-supplies policies but not the [`RequestContext`] — and a
    /// custom label missing from the catalog is a label a policy
    /// scope cannot resolve, which fails the call rather than
    /// redacting nothing.
    ///
    /// What must *not* drift travels on the audit for the same
    /// reason: the recognition context, the codec params, and that
    /// vocabulary, all as [`analyze`](Self::analyze) saw them.
    ///
    /// [`RequestContext`]: elide_provider::RequestContext
    ///
    /// `key` resolves [`HmacHash`] and [`Encrypt`]. It belongs to
    /// the caller asking for redaction rather than the process
    /// serving them, so it arrives per call and is never recorded
    /// on an audit. `None` is right when no policy names a keyed
    /// operator; a policy that names one without a key fails here.
    ///
    /// # Composition
    ///
    /// Rules attach in submission order, first match wins across
    /// the whole set. Every predicate filters by its own policy's
    /// declared labels, so a rule in policy A cannot fire on labels
    /// only B declared. Policy fallbacks attach after every
    /// policy's rules, so a coarse baseline does not shadow a
    /// later, more specific one.
    ///
    /// [`Pseudonymize`] draws from a per-policy vault: one policy
    /// pseudonymising an entity twice gets one surrogate, two
    /// policies get independent ones.
    ///
    /// # Errors
    ///
    /// Returns [`Configuration`](ErrorKind::Configuration) for a
    /// policy that cannot compile or an audit that was never
    /// analyzed, and [`MalformedInput`](ErrorKind::MalformedInput)
    /// for a document the codec cannot decode.
    ///
    /// [`Encrypt`]: elide_schema::redaction::TextRedaction::Encrypt
    /// [`HmacHash`]: elide_schema::redaction::TextRedaction::HmacHash
    /// [`Pseudonymize`]: elide_schema::redaction::TextRedaction::Pseudonymize
    pub async fn anonymize(
        &self,
        document: Document,
        policies: &[Policy],
        audit: &mut Audit,
        key: Option<&KeyConfig>,
    ) -> Result<Document> {
        let correlation_id = document.correlation_id;
        let name = document.name.clone();
        let extension = document.extension.clone();
        let content_type = document.content_type.clone();
        let mut decoded = self.decode(document, audit.codec).await?;

        // An audit describing no document was never analyzed.
        // Applying it would return the document unredacted and
        // report success, which reads exactly like "nothing to
        // redact" — so refuse instead of handing back bytes a
        // caller may believe are clean.
        if !has_documents(&audit.report) {
            return Err(Error::new(
                ErrorKind::Configuration,
                "anonymize: the audit describes no document; analyze must run first",
            ));
        }

        // The name is the join key between the two passes: it roots
        // every part path in the report, and elide matches a
        // document to its entities by it. A name the report does
        // not carry matches nothing, and elide skips that document
        // rather than failing — so without this the call returns
        // the original bytes, unredacted, with a clean `Ok`. That is
        // the same fault the check above refuses, reached by a
        // different route: an audit rebuilt against a re-derived or
        // normalised filename.
        if !names_a_document(&audit.report, name.as_str()) {
            let known: Vec<&str> = document_names(&audit.report).collect();
            return Err(Error::new(
                ErrorKind::Configuration,
                format!(
                    "anonymize: this audit describes {known:?}, not `{name}`. The \
                     document must carry the name it was analyzed under, since the \
                     report keys its entities by it.",
                ),
            ));
        }

        let orchestrator = self.provider().anonymize_orchestrator(
            &audit.context,
            &audit.recognition,
            policies,
            key,
            correlation_id,
        )?;

        // The report moves through apply and comes back mutated,
        // every entity carrying the redaction event elide stamped.
        // Swapped out and back so the caller's audit ends up holding
        // the applied report rather than the pre-apply one.
        let report = mem::replace(&mut audit.report, Report::new());
        audit.report = orchestrator.anonymize_with(&mut decoded, report).await?;

        let bytes = encode_redacted(&decoded.document)?;

        Ok(Document {
            name,
            bytes,
            extension,
            content_type,
            correlation_id,
        })
    }
}
