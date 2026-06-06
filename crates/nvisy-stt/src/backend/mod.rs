//! Backend layer: the [`SttBackend`] trait and its shipped impls.
//!
//! One trait covers every flavour of provider — hosted APIs that emit
//! a single full-clip segment (OpenAI Whisper), hosted APIs that emit
//! diarized multi-speaker segments (Deepgram, AssemblyAI), and
//! local/self-hosted inference services. Today only [`NoopBackend`]
//! (returns no segments) ships; real providers will land as
//! feature-gated siblings.

mod noop_backend;
mod stt_backend;
mod transcribed_segment;

pub use self::noop_backend::NoopBackend;
pub use self::stt_backend::{SttBackend, SttRequest, SttResponse};
pub use self::transcribed_segment::{TranscribedSegment, TranscribedWord};
