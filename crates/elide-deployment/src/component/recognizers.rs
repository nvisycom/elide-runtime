//! Recognizers: the components that find entities.
//!
//! One module per backend, each holding its deployment
//! configuration — which model, which provider, which credentials
//! — beside nothing else. `elide-provider` turns these lineups
//! into the recognizers elide runs.
//!
//! Split from [`super::enrichers`] by what a component *does*: a
//! recognizer produces entities, an enricher produces the context
//! recognizers read. Pattern recognition has no configuration (it
//! is elide's built-in catalogue), so it has no lineup here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Component;
use super::backend::{LlmBackend, NerBackend};

/// The deployment's recognizer lineups, one per backend kind.
///
/// Each lineup runs when a request selects it, by allowlist name or
/// by running the whole lineup. An empty lineup is not an error: a
/// deployment running only the pattern recognizers elide ships
/// configures none of these.
#[derive(Debug, Clone, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Recognizers {
    /// The NER lineup.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ner: Vec<Component<NerBackend>>,
    /// The LLM lineup.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub llm: Vec<Component<LlmBackend>>,
}
