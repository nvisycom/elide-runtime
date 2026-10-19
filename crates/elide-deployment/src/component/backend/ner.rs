//! NER deployment configuration.
//!
//! A request's [`ComponentSelection`] names which recognizers to
//! run, by name or by tag; every detail of *how* one runs lives
//! here, on the deployment's side. Symmetric with
//! [`super::llm`]: the operator owns model choice, connection
//! details and credentials, and a request only names which of
//! them to use.
//!
//! [`NerBackend`] is the backend enum one
//! [`Component`](crate::component::Component) carries — Bento
//! today, extensible with hosted services later.
//!
//! [`ComponentSelection`]: crate::component::ComponentSelection

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How a configured recognizer talks to its NER backend.
///
/// One rig variant today (BentoML-hosted). Extensible with
/// authenticated variants (managed NER services) later; the
/// `Mock` variant exists only when the consuming crate enables
/// the `mocks` feature, so the wire rejects
/// `kind = "mock"` in production builds.
#[derive(Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NerBackend {
    /// BentoML-hosted NER service. Engine wires the shared
    /// `elide-bentoml` client; per-recognizer URL + model come
    /// from this variant.
    Bento {
        /// Base URL of the BentoML service.
        base_url: String,
        /// Model identifier the backend should target.
        model: String,
    },
    /// Pioneer's hosted GLiNER service.
    ///
    /// Zero-shot NER: the labels a request asks for are sent with
    /// the text, so this needs no per-label model training.
    #[cfg(feature = "pioneer")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pioneer")))]
    Pioneer {
        /// API key for the Pioneer account.
        ///
        /// Read from configuration but never written back, like
        /// [`SttBackend::Deepgram`](super::SttBackend)'s: the config
        /// type is `Serialize`, and a host that dumps its effective
        /// settings — a debug endpoint, a startup log, a crash
        /// report — would otherwise put the key in plaintext
        /// wherever that lands. Serializing a config therefore
        /// yields one that cannot be loaded as-is, which is the
        /// intended trade: a secret is supplied, not round-tripped.
        #[serde(skip_serializing)]
        api_key: String,
        /// Override Pioneer's hosted endpoint.
        ///
        /// Absent uses the service's own default, which is what a
        /// deployment calling the public API wants.
        ///
        /// Not written back either, for the same reason as
        /// [`api_key`](Self::Pioneer::api_key): a URL can carry its
        /// own credential (`https://user:pw@host`), and a serialized
        /// config was already unloadable once the key was skipped.
        #[serde(default, skip_serializing)]
        base_url: Option<String>,
        /// Minimum confidence a span must reach to be returned.
        ///
        /// Absent leaves the backend's own default. Raising it
        /// trades recall for precision, which is a per-deployment
        /// judgement rather than something this runtime should pick.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        threshold: Option<f32>,
        /// Ask Pioneer not to persist the request or its response.
        ///
        /// Off by default, matching the backend: a deployment gets
        /// Pioneer's own behaviour unless it opts in.
        ///
        /// Treat this as a *request*, not a guarantee. Pioneer
        /// scopes zero-retention to "eligible use cases" without
        /// enumerating them, and it does not cover the task-model
        /// training their Trust & Safety page describes as
        /// continuing regardless. A deployment that must not send
        /// document text to a third party should not configure this
        /// backend at all.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        zero_retention: bool,
    },
    /// No-op backend; emits no entities. Test-only.
    #[cfg(feature = "mocks")]
    #[cfg_attr(docsrs, doc(cfg(feature = "mocks")))]
    Mock,
}

impl std::fmt::Debug for NerBackend {
    /// Redacts every credential-bearing field.
    ///
    /// Hand-written rather than derived for the reason
    /// [`SttBackend`](super::SttBackend)'s is: this is the
    /// rendering a panic, a `tracing` line or a debug log of the
    /// whole deployment takes, and a derived `Debug` would put the
    /// API key in all of them.
    ///
    /// `base_url` is redacted beside the key, because a URL can
    /// carry a credential of its own. Non-secret knobs print as
    /// configured, since seeing them is the point of a debug line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bento { base_url, model } => f
                .debug_struct("Bento")
                .field("base_url", base_url)
                .field("model", model)
                .finish(),
            #[cfg(feature = "pioneer")]
            Self::Pioneer {
                base_url,
                threshold,
                zero_retention,
                ..
            } => f
                .debug_struct("Pioneer")
                .field("api_key", &"***")
                .field("base_url", &base_url.as_ref().map(|_| "***"))
                .field("threshold", threshold)
                .field("zero_retention", zero_retention)
                .finish(),
            #[cfg(feature = "mocks")]
            Self::Mock => f.write_str("Mock"),
        }
    }
}

impl NerBackend {
    /// A BentoML-hosted NER service at `base_url`, targeting
    /// `model`.
    ///
    /// Saves a caller the two `.into()` calls the struct variant
    /// needs, which is the whole of what this does.
    pub fn bento(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Bento {
            base_url: base_url.into(),
            model: model.into(),
        }
    }
}

impl crate::component::Backend for NerBackend {
    /// Provider slug for the list-recognizers accessor.
    fn provider(&self) -> &'static str {
        match self {
            Self::Bento { .. } => "bento",
            #[cfg(feature = "pioneer")]
            Self::Pioneer { .. } => "pioneer",
            #[cfg(feature = "mocks")]
            Self::Mock => "mock",
        }
    }
}
