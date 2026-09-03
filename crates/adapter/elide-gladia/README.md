# elide-gladia

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/rust-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/rust-build.yml)

Gladia-backed speech-to-text backend for elide.

## Overview

An `SttBackend` over [Gladia](https://gladia.io)'s hosted transcription
API, with optional speaker diarization. A sibling to
[`elide-bentoml`](../elide-bentoml), not a part of it: Gladia has no BentoML
service in front of it. Both implement the same trait, so a deployment
picks one without either crate knowing about the other.

Transport is the [`gladia`](https://crates.io/crates/gladia) crate; this
one is the adapter between it and elide's audio vocabulary. Gladia's
pre-recorded API is asynchronous — upload, submit, then poll — but the
SDK's `transcribe_file` covers all three, which matches the
`SttBackend` contract's single `await`. Pass a pre-configured
`gladia::Client` to `from_client` to set a regional base URL, timeouts
or retries. The SDK is re-exported as `elide_gladia::gladia`, so doing
that needs no second dependency.

Timings arrive as float seconds and are rounded to whole milliseconds,
so adjacent spans do not gap. Speaker indices become `SPEAKER_00`-style
labels, matching what the self-hosted backends emit. Speaker attribution
is per utterance, not per word — Gladia does not report a mid-utterance
speaker change, so neither does this crate. Gladia's own enrichment
options (`pii_redaction`, `named_entity_recognition`, summarization) are
deliberately not exposed: redaction is elide's job, and asking the
provider to do it too would produce two disagreeing views of the same
audio.

**This backend sends raw audio to a third party.** In a redaction
pipeline that means un-redacted audio — voice being biometric personal
data under GDPR — leaves your infrastructure before anything is
redacted; the self-hosted `bento-whisper` service exists so that does
not have to happen. Retention, training opt-out, on-premise
availability, and certification status all vary by contract tier and are
described inconsistently across Gladia's own pages, so settle them in
writing before choosing this backend.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
