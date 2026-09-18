//! [`Component`]: one configured recognizer or enricher.
//!
//! Every entry in a deployment's lineup has the same shape: a name
//! for provenance, an optional description for operators, and the
//! backend that does the work, flattened onto the wire so the
//! backend's own fields sit beside them. Only the backend type
//! differs, so it is the generic parameter and everything else is
//! written once.
//!
//! Anything only one backend needs belongs on that backend, not
//! here: an LLM recognizer picks which analyzers it attaches to, so
//! that choice lives on its own backend type.

use hipstr::HipStr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One configured recognizer or enricher, generic over its backend.
///
/// The wire shape is the backend's own fields plus `name` and
/// `description`, so a config reads as one flat object rather than
/// a nested one.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// Backend selection and its per-kind fields, flattened onto
    /// this component's wire shape.
    #[serde(flatten)]
    pub backend: B,
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
