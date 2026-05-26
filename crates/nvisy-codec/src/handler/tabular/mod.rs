//! Tabular-modality codec types: [`Codable`] impl, redaction shape,
//! and the `apply_tabular_redaction` helper.
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell byte
//! offsets), distinct from text handlers that address content by
//! byte offset in a serialized stream. The per-modality capability
//! surface lives on the generic [`Handle<Tabular>`] trait in
//! [`super::handle`]. Concrete per-format implementations (CSV,
//! XLSX) live in `nvisy-formats`.
//!
//! Tabular handlers return [`TextData`] from `read` — the cell's
//! string value — so [`Codable::Data`] aliases [`TextData`] for the
//! [`Tabular`] modality.
//!
//! [`Handle<Tabular>`]: super::Handle
//! [`Tabular`]: nvisy_ontology::modality::Tabular

use nvisy_ontology::modality::Tabular;

use super::{Codable, TextData};

mod apply;
mod instruction;

pub use self::apply::apply_tabular_redaction;
pub use self::instruction::TabularRedaction;

impl Codable for Tabular {
    type Data = TextData;
    type Redaction = TabularRedaction;
}
