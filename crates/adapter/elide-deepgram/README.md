# elide-deepgram

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/rust-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/rust-build.yml)

Deepgram-backed speech-to-text backend for elide.

## Overview

An `SttBackend` over [Deepgram](https://deepgram.com)'s hosted
transcription API, with optional speaker diarization. A sibling to
[`elide-bentoml`](../elide-bentoml), not a part of it: Deepgram has no
BentoML service in front of it. Both implement the same trait, so a
deployment picks one without either crate knowing about the other.

Transport is the official [`deepgram`](https://crates.io/crates/deepgram)
crate; this one is the adapter between it and elide's audio vocabulary.
The pre-recorded API is synchronous — one call returns the transcript,
which matches the `SttBackend` contract's single `await`. Pass a
pre-configured `deepgram::Deepgram` to `from_client` for a regional or
self-hosted endpoint; the SDK is re-exported as
`elide_deepgram::deepgram`, so doing that needs no second dependency.

The request always asks for `utterances`, which is what turns Deepgram's
flat per-channel transcript into the segments elide models; a response
without them is treated as a protocol error rather than an empty
transcript. Timings arrive as float seconds and are rounded to whole
milliseconds, so adjacent spans do not gap. Speaker indices become
`SPEAKER_00`-style labels, matching what the self-hosted backend emits.
Deepgram's own `redact` option is deliberately not exposed: redaction is
elide's job, and a provider-redacted transcript would no longer match the
audio whose offsets elide needs.

**This backend sends raw audio to a third party.** In a redaction
pipeline that means un-redacted audio — personal data, and biometric
data under GDPR Article 4(14) where it is processed to identify or
authenticate a speaker — leaves your infrastructure before anything is
redacted;
the self-hosted `bento-whisper` service exists so that does not have to
happen. Deepgram offers a BAA on request, an explicit `mip_opt_out`
parameter to keep submitted audio out of model training, and an EU
endpoint — but which of those apply is a matter of contract tier, so
settle them in writing before this backend carries production audio.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
