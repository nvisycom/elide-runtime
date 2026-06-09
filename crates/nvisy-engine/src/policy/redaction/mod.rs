//! Per-modality redaction operator specs — the closed wire vocabulary
//! a policy author can choose from inside a `redact` rule.
//!
//! Each modality has its own enum because the operator catalogue
//! differs by modality. Text gets the full toolkit built-in set
//! (`Replace`, `Mask`, `Hash`, `Redact`, `Keep`) plus a `Custom`
//! escape hatch. Image / Audio / Tabular currently expose only
//! `Custom` — the toolkit ships no built-in operators for those
//! modalities yet.
//!
//! The split between *this* enum (the spec) and the toolkit's
//! [`Anonymizer<M>`] trait is intentional: the spec is the
//! serialisable, author-facing wire shape; instantiating it at apply
//! time produces the runtime operator instance.
//!
//! [`Anonymizer<M>`]: nvisy_toolkit::redaction::Anonymizer

mod any;
mod audio;
mod image;
mod instantiate;
mod tabular;
mod text;

pub use nvisy_toolkit::redaction::builtin::HashAlgorithm;

pub use self::any::AnyRedaction;
pub use self::audio::AudioRedaction;
pub use self::image::ImageRedaction;
pub use self::instantiate::Instantiate;
pub use self::tabular::TabularRedaction;
pub use self::text::TextRedaction;
