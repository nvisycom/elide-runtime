//! Attach the STT enricher to an audio-modality [`Analyzer`].
//!
//! Audio-modality only. The deployment's `Bento` backend returns
//! a clean "not wired yet" error until `elide-bento` ships a
//! `BentoStt` client.

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::enrichment::stt::{MockBackend as MockSttBackend, SttEnricher};
use elide_core::modality::audio::Audio;
use elide_core::{Error, ErrorKind, Result};

use crate::provider::stt::SttBackend;

/// Attach an [`SttEnricher`] for the audio modality.
///
/// [`SttEnricher`]: elide::enrichment::stt::SttEnricher
pub(in crate::analyzer) fn attach(
    analyzer: Analyzer<Audio>,
    backend: &SttBackend,
) -> Result<Analyzer<Audio>> {
    #[cfg(not(feature = "test-utils"))]
    let _ = analyzer;
    match backend {
        SttBackend::Bento { .. } => Err(Error::new(
            ErrorKind::CapabilityUnavailable,
            "analyzer compile: BentoML STT backend needs an elide-bento `BentoStt` \
             client; not wired into the compile surface yet",
        )),
        #[cfg(feature = "test-utils")]
        SttBackend::Mock => Ok(analyzer.with_enricher(SttEnricher::new(MockSttBackend))),
    }
}
