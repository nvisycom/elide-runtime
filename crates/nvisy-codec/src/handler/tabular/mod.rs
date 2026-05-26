//! Tabular-modality wire types: [`Codable`] impl + redaction shape.
//!
//! Tabular handlers address content by cell coordinate
//! ([`Tabular`] = row + column, optionally with intra-cell byte
//! offsets), distinct from text handlers that address content by
//! byte offset in a serialized stream. The per-modality capability
//! surface lives on the generic [`Handle<Tabular>`] trait in
//! [`crate::core`]. Concrete per-format implementations (CSV, XLSX)
//! live in `nvisy-formats`; cells are strings, so they share the
//! text crate's redaction helper.
//!
//! Tabular handlers return [`TextData`] from `read` — the cell's
//! string value — so [`Codable::Data`] aliases [`TextData`] for the
//! [`Tabular`] modality.
//!
//! [`Handle<Tabular>`]: crate::core::Handle
//! [`Tabular`]: nvisy_ontology::modality::Tabular

use nvisy_ontology::modality::Tabular;

use super::TextData;
use crate::core::Codable;

mod instruction;

pub use self::instruction::TabularRedaction;

impl Codable for Tabular {
    type Data = TextData;
    type Redaction = TabularRedaction;
}
