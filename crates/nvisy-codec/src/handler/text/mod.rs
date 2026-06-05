//! Text-modality wire types: [`Codable`] impl.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`crate::core`]. Concrete per-format
//! implementations (TXT, JSON, Markdown, HTML) live in
//! `nvisy-formats`; the byte-level `&mut String` redaction helper
//! they share lives there too.
//!
//! Replacements written during [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::TextReplacement`] — codec depends on
//! core, not the reverse.
//!
//! [`Handle<Text>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact

use nvisy_core::modality::{ModalityKind, Text};

use crate::core::Codable;

impl Codable for Text {
    const KIND: ModalityKind = ModalityKind::Text;
}
