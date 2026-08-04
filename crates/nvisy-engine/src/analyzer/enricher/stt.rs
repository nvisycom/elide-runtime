//! Attach the STT enricher to an audio-modality [`Analyzer`].
//!
//! Audio-modality only. The deployment's `Bento` backend returns
//! a clean "not wired yet" error until `elide-bento` ships a
//! `BentoStt` client; unknown non-exhaustive variants surface as
//! Validation.

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::enrichment::stt::{MockBackend as MockSttBackend, SttEnricher};
use elide_core::modality::audio::Audio;
use elide_core::{Error, ErrorKind};
use nvisy_schema::plan::{SttBackendParams, SttEnricherParams};

/// Attach an [`SttEnricher`] for the audio modality.
///
/// [`SttEnricher`]: elide::enrichment::stt::SttEnricher
pub(in crate::analyzer) fn attach(
    analyzer: Analyzer<Audio>,
    spec: &SttEnricherParams,
) -> Result<Analyzer<Audio>, Error> {
    #[cfg(not(feature = "test-utils"))]
    let _ = analyzer;
    match &spec.backend {
        SttBackendParams::Bento { .. } => Err(Error::new(
            ErrorKind::CapabilityUnavailable,
            "analyzer compile: BentoML STT backend needs an elide-bento `BentoStt` \
             client; not wired into the compile surface yet",
        )),
        #[cfg(feature = "test-utils")]
        SttBackendParams::Mock => Ok(analyzer.with_enricher(SttEnricher::new(MockSttBackend))),
        // `SttBackendParams` is `#[non_exhaustive]`. Unknown
        // variants surface as Validation.
        _ => Err(Error::new(
            ErrorKind::CapabilityUnavailable,
            "analyzer compile: STT enricher uses a backend kind this engine binary \
             doesn't understand; upgrade the engine or downgrade the config",
        )),
    }
}
