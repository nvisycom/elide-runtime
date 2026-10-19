//! What a deployment wires, and which of it a request may run.
//!
//! The vocabulary both sides of a request speak about components: a
//! [`Component`] names one backend a deployment configured,
//! [`Recognizers`]/[`Enrichers`] are the lineups it groups them
//! into, and [`Availability`]/[`ComponentSelection`] say which of
//! them a caller may reach and which this request asked for.
//!
//! Deployment-owned, so it lives beside the factories that build
//! from it rather than in the request schema: these carry API keys
//! and base URLs, which a request never sends.

mod availability;
mod enrichers;
mod recognizers;

/// The hosted backends a deployment can name, one module per kind.
pub mod backend {
    mod llm;
    mod ner;
    mod ocr;
    mod stt;

    pub use self::llm::{
        AttachTo, AuthenticatedProvider, LlmBackend, LlmSource, UnauthenticatedProvider,
    };
    pub use self::ner::NerBackend;
    pub use self::ocr::OcrBackend;
    pub use self::stt::SttBackend;
}

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::availability::{Availability, ComponentSelection, ResolvedComponents, Resolver};
pub use self::backend::{
    AttachTo, AuthenticatedProvider, LlmBackend, LlmSource, NerBackend, OcrBackend, SttBackend,
    UnauthenticatedProvider,
};
pub use self::enrichers::Enrichers;
pub use self::recognizers::Recognizers;
use crate::feature::RateLimit;

/// One configured recognizer or enricher, generic over its backend.
///
/// The wire shape is the backend's own fields plus `name` and
/// `tags`, so a config reads as one flat object rather than a
/// nested one.
///
/// Deliberately *not* `deny_unknown_fields`, unlike the lineups
/// that hold these: serde rejects that combination with
/// `flatten`, because the flattened backend's own fields reach
/// this struct first and would be refused as unknown. A valid
/// document would fail to load.
///
/// The cost is that a misspelled optional key here — `tagz` for
/// `tags` — is ignored rather than refused. The lineup around it
/// still catches a misspelled *lineup*, and the backend enums
/// catch a misspelled backend field, so what slips through is
/// narrow: the two optional keys on this struct.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "B: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "B: JsonSchema")]
pub struct Component<B> {
    /// Component name. Surfaces on the per-entity provenance trail
    /// so audits can attribute detections to a specific configured
    /// component, and is what a request's allowlist selects by.
    /// Must be unique across its lineup.
    #[schemars(with = "String")]
    pub name: HipStr<'static>,
    /// Deployment-defined groupings this component belongs to.
    ///
    /// Free-form: `"general"`, `"medical"`, `"advanced"`,
    /// `"eu-only"` — whatever a deployment sorts its lineups by.
    /// The runtime matches on them and assigns them no meaning, so
    /// a deployment can restrict or select a family of components
    /// without naming each one, and adding a component to a family
    /// needs no change anywhere else.
    ///
    /// Empty by default, which places a component in no family: it
    /// is then reachable only by its own name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub tags: Vec<HipStr<'static>>,
    /// How often this component's backend may be called.
    ///
    /// Absent means unlimited: the backend's own quota is the one
    /// that matters, and a second invisible limit would be harder
    /// to diagnose than no limit at all.
    ///
    /// Accepts `rate_limit` as well as the camelCase `rateLimit`:
    /// the surrounding TOML is snake_case throughout (`base_url`,
    /// `api_key`, `zero_retention`), because those come from
    /// backend enums that rename to snake, so an operator writing
    /// `rate_limit` here is following the file's own convention.
    /// Without the alias that spelling parsed as an unknown key and
    /// the limit was silently dropped.
    #[serde(default, alias = "rate_limit", skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,
    /// Backend selection and its per-kind fields, flattened onto
    /// this component's wire shape.
    #[serde(flatten)]
    pub backend: B,
}

impl<B> Component<B> {
    /// One component called `name`, running `backend`, in no
    /// families and with no rate limit.
    ///
    /// The optional parts are set with
    /// [`with_tags`](Self::with_tags) and
    /// [`with_rate_limit`](Self::with_rate_limit), so adding another
    /// does not break every call site the way a struct literal
    /// does.
    pub fn new(name: impl Into<HipStr<'static>>, backend: B) -> Self {
        Self {
            name: name.into(),
            tags: Vec::new(),
            rate_limit: None,
            backend,
        }
    }

    /// Place this component in the given deployment-defined
    /// families.
    #[must_use]
    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<HipStr<'static>>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Limit how often this component's backend may be called.
    #[must_use]
    pub fn with_rate_limit(mut self, rate_limit: RateLimit) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    /// Whether this component answers to `key`, by its own name or
    /// by one of its tags.
    ///
    /// The question both [`Availability`] and [`ComponentSelection`]
    /// ask, which is why it lives here rather than in either: a
    /// component knows what it answers to, and neither of those
    /// should reach into its fields to decide.
    #[must_use]
    pub fn matches(&self, key: &str) -> bool {
        self.name == key || self.tags.iter().any(|tag| tag == key)
    }

    /// Whether this component answers to any of `keys`.
    #[must_use]
    pub fn matches_any<K: AsRef<str>>(&self, keys: &[K]) -> bool {
        keys.iter().any(|key| self.matches(key.as_ref()))
    }

    /// Check this component describes something the runtime can
    /// build.
    ///
    /// # Errors
    ///
    /// Returns [`Invalid`](crate::ErrorKind::Invalid) if the rate
    /// limit would never admit a call.
    pub fn validate(&self) -> crate::Result<()> {
        if let Some(rate_limit) = &self.rate_limit {
            rate_limit.validate(self.name.as_str())?;
        }
        Ok(())
    }
}

/// A backend that can name the provider behind it.
///
/// Implemented by every backend a [`Component`] can carry, so a
/// caller listing what a deployment registered reads one slug per
/// component without matching on the backend kind.
pub trait Backend {
    /// Provider slug: `"bento"`, `"openai"`, `"mock"`, and so on.
    fn provider(&self) -> &'static str;
}
