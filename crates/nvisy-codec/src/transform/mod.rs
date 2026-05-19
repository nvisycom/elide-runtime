//! Redaction transform primitives.
//!
//! [`Redactions`] groups per-modality instructions by their target
//! location and enforces an overlap [`ConflictPolicy`] on insert. The
//! per-modality [`TextTransform`], [`ImageTransform`], [`AudioTransform`],
//! and [`TabularTransform`] traits are blanket-implemented over the
//! corresponding handler capability traits ([`TextHandler`],
//! [`ImageHandler`], [`AudioHandler`], [`TabularHandler`]) and own the
//! iteration over a [`Redactions`] batch, dispatching each
//! `(location, redaction)` pair to the handler's narrow `redact_at`
//! hook.
//!
//! Buffer-mutation helpers live with their consumers under
//! `crate::handler::{text,image,audio,tabular}` since each
//! `apply_*_redaction` is an implementation detail of one handler
//! family.
//!
//! [`Redactions`]: crate::transform::Redactions
//! [`ConflictPolicy`]: crate::transform::ConflictPolicy
//! [`TextHandler`]: crate::handler::TextHandler
//! [`ImageHandler`]: crate::handler::ImageHandler
//! [`AudioHandler`]: crate::handler::AudioHandler
//! [`TabularHandler`]: crate::handler::TabularHandler

mod audio;
mod image;
mod policy;
mod redactions;
mod tabular;
mod text;

pub use nvisy_ontology::entity::Mergeable;

pub use self::audio::{AudioOutput, AudioRedaction, AudioTransform};
pub use self::image::{ImageOutput, ImageRedaction, ImageTransform};
pub use self::policy::{ConflictPolicy, InsertError};
pub use self::redactions::Redactions;
pub use self::tabular::{TabularRedaction, TabularTransform};
pub use self::text::{TextOutput, TextRedaction, TextTransform};
