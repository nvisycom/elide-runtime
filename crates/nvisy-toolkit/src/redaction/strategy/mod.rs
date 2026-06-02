//! Per-modality redaction strategies.
//!
//! Each modality exposes one strategy enum
//! ([`TextStrategy`], [`ImageStrategy`], [`AudioStrategy`],
//! [`TabularStrategy`]) pairing a redaction method with its
//! parameters. The modality's [`Redactable::Strategy`] associated
//! type points at the appropriate one; consumers parameterise their
//! own types over `M::Strategy`.
//!
//! [`Redactable::Strategy`]: super::Redactable::Strategy

mod audio;
mod image;
mod tabular;
mod text;

pub use self::audio::{AudioMethodTag, AudioStrategy};
pub use self::image::{ImageMethodTag, ImageStrategy};
pub use self::tabular::TabularStrategy;
pub use self::text::TextStrategy;
