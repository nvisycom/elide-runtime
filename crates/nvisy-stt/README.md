# nvisy-stt

Speech-to-text extractor for the Nvisy runtime. Mirrors the layout of
[`nvisy-ner`][ner] and [`nvisy-ocr`][ocr]: a per-call [`SttBackend`] trait,
shipped implementations behind that trait, and an [`SttExtractor`] that
type-erases the backend and plugs into the toolkit pipeline as
[`Extractor<Audio>`][extractor].

## Backends

| Backend         | Status              |
|-----------------|---------------------|
| `NoopBackend`   | shipped             |

`NoopBackend` returns an empty [`Transcription`] for every request and
acts as both the default when STT isn't configured and the test stub.
Real provider backends (OpenAI Whisper, Deepgram, AssemblyAI) plug in
later as feature-gated siblings — the trait is the integration point.

## Transcription shape

The extractor produces a [`Transcription`] with an ordered list of
[`TranscribedSegment`]s. Each segment carries a `TimeSpan`, the
recognised text, and optional fields backends may or may not populate
(`speaker_id` for diarized providers, `language` for code-switching
providers, `confidence`, word-level breakdown). Backends without
diarization leave `speaker_id = None` — the field exists so
diarization-capable providers can plug in without a shape change.

[ner]: https://docs.rs/nvisy-ner
[ocr]: https://docs.rs/nvisy-ocr
[extractor]: https://docs.rs/nvisy-core/latest/nvisy_core/extraction/trait.Extractor.html
