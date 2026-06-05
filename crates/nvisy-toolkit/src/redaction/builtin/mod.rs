//! Built-in [`Anonymizer<M>`] implementations shipped with the
//! toolkit.
//!
//! Each operator is a typed Rust struct; consumers construct it with
//! the parameters they want and register the instance against the
//! [`EntityKind`]s it should run for.
//!
//! [`Anonymizer<M>`]: super::Anonymizer
//! [`EntityKind`]: nvisy_core::entity::EntityKind

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
mod encrypt;
mod hash;
mod keep;
mod mask;
mod redact;
mod replace;
mod text_value;

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
pub use self::encrypt::{Decrypt, Encrypt};
pub use self::hash::{Hash, HashAlgorithm};
pub use self::keep::Keep;
pub use self::mask::Mask;
pub use self::redact::Redact;
pub use self::replace::Replace;
