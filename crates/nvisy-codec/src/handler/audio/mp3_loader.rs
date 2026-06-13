//! MP3 loader: wraps raw audio bytes into a [`Mp3Handler`].

use nvisy_core::Error;
use nvisy_core::modality::Audio;

use super::Mp3Handler;
use super::mp3_codec::probe_channels;
use crate::Loader;
use crate::content::{ContentData, ContentSource};

const TARGET: &str = "nvisy_codec::handler::audio::mp3_loader";

/// Loader that wraps raw MP3 bytes. Produces one [`Mp3Handler`] per input.
#[derive(Debug, Default)]
pub struct Mp3Loader;

#[async_trait::async_trait]
impl Loader<Audio> for Mp3Loader {
    type Handler = Mp3Handler;

    /// Decode the loader content into a handler.
    ///
    /// MP3 redaction round-trips through `mp3lame-encoder`, which
    /// only supports 1 or 2 channels. Reject anything wider here so
    /// the failure is surfaced at load time (with a clear message)
    /// rather than later from the redact path. Silent downmixing
    /// would quietly edit the *unredacted* audio.
    #[tracing::instrument(name = "mp3.decode", skip_all, fields(input_bytes))]
    async fn decode(&self, content: ContentData) -> Result<Mp3Handler, Error> {
        tracing::Span::current().record("input_bytes", content.to_bytes().len());
        let parent = content.content_source;
        let bytes = content.to_bytes();

        let channels = probe_channels(&bytes)?;
        if channels > 2 {
            return Err(Error::validation(
                format!(
                    "MP3 has {channels} channels; only mono and stereo MP3s are supported \
                     (downmixing would silently edit unredacted audio)"
                ),
                TARGET,
            ));
        }

        let source = ContentSource::new().with_parent(&parent);
        Ok(Mp3Handler::new(bytes).with_source(source))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::super::mp3_codec::encode_from_pcm;
    use super::*;

    /// Mint a tiny stereo MP3 from sample-level data so the loader's
    /// channel-count gate has something real to probe. We can't easily
    /// fixture the >2-channel rejection path here — LAME only encodes
    /// mono/stereo, so producing a 6-channel MP3 in-test would need an
    /// external encoder. That arm is covered by `probe_channels` unit
    /// tests upstream of the loader.
    fn fixture_stereo_mp3() -> Bytes {
        let samples = vec![0f32; 16_000 * 2]; // 1s stereo silence
        let encoded = encode_from_pcm(&samples, 16_000, 2, 64_000).expect("encode fixture");
        Bytes::from(encoded)
    }

    #[tokio::test]
    async fn accepts_stereo_mp3() {
        let bytes = fixture_stereo_mp3();
        let content = ContentData::new(ContentSource::new(), bytes);
        let handler = Mp3Loader.decode(content).await;
        handler.expect("stereo MP3 should load");
    }

    #[tokio::test]
    async fn rejects_garbage_bytes() {
        let content = ContentData::new(
            ContentSource::new(),
            Bytes::from_static(b"definitely not an mp3"),
        );
        let err = Mp3Loader.decode(content).await.unwrap_err();
        assert!(err.to_string().contains("MP3 probe failed"));
    }
}
