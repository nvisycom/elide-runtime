//! [`Engine`]: the stateless pipeline over [`elide`].
//!
//! Its only long-lived state is a [`Provider`]: the deployment's
//! configuration, already built. It owns the [`FormatRegistry`]
//! documents decode through and the recognizer and enricher
//! lineups. [`Engine`] clones cheaply (`Arc` under the hood).
//!
//! ## Per-document verbs
//!
//! - [`analyze`](Engine::analyze) decodes raw bytes, runs
//!   detection, and projects the report — body and every container
//!   part — onto an [`Audit`]. In [`analyze`](mod@analyze), with
//!   its re-run counterpart.
//! - [`anonymize`](Engine::anonymize) decodes those bytes again,
//!   layers the policy set onto each modality's anonymizer, and
//!   returns the re-encoded [`Document`](crate::file::Document). In
//!   [`anonymize`](mod@anonymize).
//!
//! Both build a fresh [`Orchestrator`] per call: a small map of
//! trait objects keyed by modality `TypeId`. Building per call is
//! what lets policies re-resolve per document without mutating
//! shared state.
//!
//! The rest is what the verbs lean on, one subject per module:
//! [`decode`](mod@decode) resolves bytes and a codec into
//! addressable content, [`report`](mod@report) holds the probes
//! both verbs refuse on and the closing re-encode, and
//! [`wire`](mod@wire) reads an [`Audit`] back from serialized form.
//!
//! ## What may change between the two calls
//!
//! Recognition inputs have two owners, and the split decides what
//! may change between the calls. Policies bring the scopes and the
//! rules, re-compiled afresh each call, so governance stays live:
//! edit a policy in the gap and it takes effect. Everything the
//! caller asserted about *this* run travels on the audit instead —
//! languages, jurisdictions and tags on [`Audit::context`], the
//! labels the request introduced on [`Audit::recognition`] —
//! because a caller re-supplies policies but not the request, and
//! anonymize must compile against the vocabulary analyze detected
//! with.
//!
//! Hosts hold the [`Audit`] between the two calls however they see
//! fit — in memory, a run store, a reviewer UI — and hand it back
//! with any reviewer decisions folded in.
//!
//! [`Audit`]: crate::Audit
//! [`Audit::context`]: crate::Audit::context
//! [`Audit::recognition`]: crate::Audit::recognition
//! [`FormatRegistry`]: elide::codec::FormatRegistry
//! [`Orchestrator`]: elide::Orchestrator
//! [`Provider`]: elide_provider::Provider

mod analyze;
mod anonymize;
mod decode;
mod report;
mod wire;

use elide::codec::FormatRegistry;
use elide_provider::Provider;

use crate::audit::RegisteredComponents;

/// Cheaply-cloneable pipeline adapter over [`elide`].
///
/// Wraps a [`Provider`](elide_provider::Provider) — the codec
/// registry and the deployment's recognizer and enricher lineups —
/// with the two verbs that run documents through it.
#[derive(Clone)]
pub struct Engine {
    provider: Provider,
}

impl Engine {
    /// An engine over `provider`'s configuration.
    ///
    /// A [`Provider`](elide_provider::Provider) holds what a
    /// deployment decides once; an engine adds the verbs that run
    /// documents through it. One provider can back many engines,
    /// and cloning either is cheap.
    #[must_use]
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    /// The configuration this engine runs on.
    #[must_use]
    pub fn provider(&self) -> &Provider {
        &self.provider
    }

    /// The codec registry the engine decodes documents through.
    pub fn formats(&self) -> &FormatRegistry {
        self.provider.formats()
    }

    /// Every recognizer and enricher this engine has registered,
    /// each lineup in configuration order.
    pub fn components(&self) -> RegisteredComponents {
        let recognizers = self.provider.recognizers();
        let enrichers = self.provider.enrichers();
        RegisteredComponents {
            ner: recognizers.ner.iter().map(Into::into).collect(),
            llm: recognizers.llm.iter().map(Into::into).collect(),
            ocr: enrichers.ocr.iter().map(Into::into).collect(),
            stt: enrichers.stt.iter().map(Into::into).collect(),
        }
    }
}
