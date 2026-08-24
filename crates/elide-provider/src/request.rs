//! [`RequestContext`]: what one request supplies beyond its
//! policies.
//!
//! A [`Provider`] holds what a deployment decides once. This holds
//! what the *caller* decides, per request, and it is separate for
//! the same reason: one engine serves many callers, so anything
//! belonging to the caller cannot sit on the provider.
//!
//! Today that is the cryptographic key. A key belongs to whoever
//! asked for redaction, not to the process serving them: putting
//! one on the provider would mean rebuilding the provider per
//! tenant, and would make it impossible to run two tenants through
//! the same one.
//!
//! Anything else with that shape belongs here too — a per-tenant
//! pseudonym vault, a caller-supplied surrogate seed, a request
//! deadline. The test is whether two callers sharing one provider
//! could disagree about it; if they could, it is not deployment
//! configuration.
//!
//! `correlation_id` is deliberately *not* here, despite arriving
//! per request: it identifies a *document*, not a request. It rides
//! on the document in and is stamped onto the redacted document
//! out, so the two are linked. A copy here could name a different
//! id than the document it travels with, and only one of them would
//! reach the output.
//!
//! The test above still decides it. Two callers sharing a provider
//! cannot disagree about a document's identity, because each brings
//! its own document.
//!
//! [`Provider`]: crate::Provider

use crate::KeyConfig;

/// What one request supplies to the redaction path beyond its
/// policies.
///
/// Empty by default: a request whose policies name no keyed
/// operator needs nothing here.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct RequestContext {
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

    /// The same context, redacting with `key`.
    #[must_use]
    pub fn with_key(mut self, key: KeyConfig) -> Self {
        self.key = Some(key);
        self
    }
}
