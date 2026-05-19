//! Redaction transform primitives.
//!
//! [`Redactions`] groups per-modality instructions by their target
//! span identity and enforces an overlap [`ConflictPolicy`] on insert.
//! Handler capability traits ([`TextHandler`], [`ImageHandler`],
//! [`AudioHandler`]) consume these collections directly via their
//! `redact` methods.
//!
//! [`Redactions`]: crate::transform::Redactions
//! [`ConflictPolicy`]: crate::transform::ConflictPolicy
//! [`TextHandler`]: crate::handler::TextHandler
//! [`ImageHandler`]: crate::handler::ImageHandler
//! [`AudioHandler`]: crate::handler::AudioHandler

mod audio;
mod image;
mod mergeable;
mod policy;
mod redactions;
mod tabular;
mod text;

pub use self::audio::{AudioOutput, AudioRedaction};
pub(crate) use self::image::ImageOps;
pub use self::image::{ImageOutput, ImageRedaction};
pub use self::mergeable::Mergeable;
pub use self::policy::{ConflictPolicy, InsertError};
pub use self::redactions::Redactions;
pub use self::tabular::TabularRedaction;
pub use self::text::{TextOutput, TextRedaction};
