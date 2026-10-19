//! Outgoing request options for the pre-recorded transcription call.

use deepgram::common::options::{Language, Model, Options};

/// Build the options for one transcription.
///
/// `utterances` is always on: it is what turns Deepgram's flat per-channel
/// transcript into the segments elide's [`TranscriptSegment`] models. Its
/// absence in a response is treated as a protocol error for that reason.
///
/// Deepgram's own `redact` option is deliberately not exposed: redaction is
/// elide's job, and asking the provider to redact the same audio would
/// produce two disagreeing views of it — and would corrupt the offsets
/// elide needs, since the returned transcript would no longer match the
/// audio it describes.
///
/// [`TranscriptSegment`]: elide_audio::modality::TranscriptSegment
pub(super) fn options(model: &Model, diarize: bool, language: Option<&str>) -> Options {
    let mut builder = Options::builder()
        .model(model.clone())
        .utterances(true)
        .punctuate(true)
        .diarize(diarize);

    // An asserted language is a hint; omitted means Deepgram detects it.
    // `Language::Other` carries any BCP-47 tag the SDK has no variant for.
    if let Some(tag) = language {
        builder = builder.language(Language::Other(tag.to_owned()));
    }
    builder.build()
}
