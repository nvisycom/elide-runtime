//! Shared provider dispatch enums for audio models.

mod provider;

pub use provider::TranscribeProvider;
pub(crate) use provider::TranscribeModels;

#[cfg(feature = "audio")]
pub use provider::AudioGenProvider;
#[cfg(feature = "audio")]
pub(crate) use provider::AudioGenModels;
