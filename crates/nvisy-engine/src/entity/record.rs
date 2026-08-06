//! One recognised entity plus the optional reviewer override.

use elide_core::entity::Entity;
use elide_core::modality::Modality;
use nvisy_schema::policy::redaction::ModalityRedactions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One recognized entity plus the optional reviewer override.
///
/// The bound mirrors elide's [`Entity<M>`]: serialization needs
/// `M::Location` and `M::Data` (de)serializable, and JsonSchema
/// derivation needs them schema-able. All four modalities elide
/// ships satisfy these under the `serde` + `schema` features.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "M::Location: Serialize + for<'a> Deserialize<'a>, \
                  M::Data: Serialize + for<'a> Deserialize<'a>")]
#[schemars(bound = "M: JsonSchema, M::Location: JsonSchema, M::Data: JsonSchema")]
#[schemars(rename = "{M}EntityRecord")]
pub struct EntityRecord<M: Modality> {
    /// The elide entity, as recognition produced it.
    pub entity: Entity<M>,
    /// Reviewer-supplied redaction override.
    ///
    /// `None` means "use the matching policy rule's decision";
    /// `Some(...)` overrides that rule for this specific entity
    /// at apply time. Reviewer overrides take precedence over
    /// every policy rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ModalityRedactions>,
}

impl<M: Modality> EntityRecord<M> {
    /// New record over `entity`, no review override.
    pub fn new(entity: Entity<M>) -> Self {
        Self {
            entity,
            review: None,
        }
    }
}
