//! Built-in [`Anonymizer<M>`] implementations shipped with the
//! toolkit.
//!
//! Each operator is a typed Rust struct; consumers construct it
//! with the parameters they want and register the instance
//! against the [`EntityLabelRef`]s it should run for.
//!
//! [`Anonymizer<M>`]: crate::redaction::Anonymizer
//! [`EntityLabelRef`]: nvisy_core::entity::EntityLabelRef

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
pub(crate) mod encrypt;
mod hash;
mod keep;
mod mask;
mod redact;
mod replace;

#[cfg(feature = "encrypt")]
#[cfg_attr(docsrs, doc(cfg(feature = "encrypt")))]
pub use self::encrypt::Encrypt;
pub use self::hash::{Hash, HashAlgorithm};
pub use self::keep::Keep;
pub use self::mask::Mask;
pub use self::redact::Redact;
pub use self::replace::Replace;
