//! [`RequestContext`]: what one request supplies beyond its
//! policies.
//!
//! A [`Provider`] holds what a deployment decides once. This holds
//! what the *caller* decides, per request, and it is separate for
//! the same reason: one engine serves many callers, so anything
//! belonging to the caller cannot sit on the provider.
//!
//! Three things have that shape today: what the caller asserts
//! about the document ([`DocumentContext`]), how its bytes decode
//! ([`CodecParams`]), and the cryptographic key. The key is the
//! clearest case — it belongs to whoever asked for redaction, not
//! to the process serving them: putting one on the provider would
//! mean rebuilding it per tenant, and would make it impossible to
//! run two tenants through the same one.
//!
//! The three do not share a lifecycle, and the split matters.
//! `context` and `codec` are recorded onto the audit, because
//! anonymize must recognize against the same vocabulary and decode
//! to the same bytes analyze did. The key is never recorded: it is
//! a secret, and it is supplied again at anonymize where the
//! operators actually run.
//!
//! Anything else with that shape belongs here too — a per-tenant
//! pseudonym vault, a caller-supplied surrogate seed, a request
//! deadline. The test is whether two callers sharing one provider
//! could disagree about it; if they could, it is not deployment
//! configuration.
//!
//! `correlation_id` is deliberately *not* here. Every call already
//! takes a [`Document`], and that is where it lives: one id, on the
//! thing it identifies.
//!
//! [`Document`]: https://docs.rs/elide-pipeline
//!
//! [`Provider`]: crate::Provider

use super::{CodecParams, DocumentContext, KeyConfig};

/// What one request supplies beyond its policies.
///
/// Empty by default: a caller asserting nothing about the document,
/// wanting the codec's own behaviour, and naming no keyed operator
/// passes [`RequestContext::new`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RequestContext {
    /// What the caller asserts about the document: languages,
    /// jurisdictions, free-form classification.
    ///
    /// Drives recognition. Recorded onto the audit so anonymize
    /// compiles against the same vocabulary analyze used.
    pub context: DocumentContext,
    /// How the document's bytes decode.
    ///
    /// Recorded onto the audit so anonymize decodes identically:
    /// entity offsets are stored against the first decode.
    pub codec: CodecParams,
    /// The key `HmacHash` and `Encrypt` resolve through.
    ///
    /// `None` when the caller supplied none. A policy naming
    /// either operator then fails at request-compile time, saying
    /// which policy and which operator, rather than redacting with
    /// some default key.
    pub key: Option<KeyConfig>,
}

impl RequestContext {
    /// A context supplying nothing.
    ///
    /// The starting point for the builders below;
    /// `#[non_exhaustive]` means a caller constructs one this way
    /// rather than with a struct literal, so a field added later
    /// does not break them.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same context, asserting `context` about the document.
    #[must_use]
    pub fn with_context(mut self, context: DocumentContext) -> Self {
        self.context = context;
        self
    }

    /// The same context, decoding under `codec`.
    #[must_use]
    pub fn with_codec(mut self, codec: CodecParams) -> Self {
        self.codec = codec;
        self
    }

    /// The same context, redacting with `key`.
    #[must_use]
    pub fn with_key(mut self, key: KeyConfig) -> Self {
        self.key = Some(key);
        self
    }
}
