# STT: speech with word and segment timings

`elide-stt` transcribes audio into text with time spans. The Rust runtime uses
those timings to relate text recognition back to the audio that may need
redaction. Optional speaker labels provide grouping within a transcript. See the
[shared service architecture](README.md) for the HTTP and deployment boundaries.

## Pipeline choice

The service uses [faster-whisper](https://github.com/SYSTRAN/faster-whisper) for
transcription and its native word timestamps. Optional
[pyannote diarization](https://huggingface.co/pyannote/speaker-diarization-community-1)
adds speaker turns. These are assembled directly so transcription, word timing,
and speaker labeling have explicit configuration and loading boundaries.

The default transcription model is `large-v3-turbo`, loaded once at startup.
`ELIDE_BENTO_MODEL_NAME` selects the model; a nonempty
`ELIDE_BENTO_MODEL_PATH` directory takes precedence. Device defaults to CPU,
with `int8` compute; CUDA defaults to `float16`. An explicit compute type
can override that choice. Word timestamps default to enabled.

The design favors a deployment that can run on CPU and return timed words
without another alignment model. Model accuracy and timing quality still need
evaluation on the deployment's languages, recording conditions, and vocabulary.

## Request and bounded decoding

`POST /transcribe` accepts one object under `request`, containing base64 `audio`
and optional language and filename hints. The current decoder reads the audio
bytes directly; the filename is accepted by the contract but does not drive a
separate decoder-selection path.

The endpoint is synchronous and does not use adaptive batching. Clip lengths
vary substantially, and coalescing clips would couple short requests to a long
transcription. The service declares four CPUs and a 900-second request timeout;
these are execution settings, not a guarantee that every allowed clip will
complete on every deployment.

The [pipeline](../../packages/elide-stt/src/elide_stt/inference/pipeline.py)
applies two independent budgets. The default payload budget is 512 MiB,
checked as an equivalent base64-length bound before allocating decoded bytes.
The default duration budget is 3,600 seconds. Decoding produces mono, 16 kHz
floating-point audio and reads at most the configured duration plus one second.
The extra second makes overlong input detectable without decoding an entire
arbitrarily long recording.

Byte size does not determine duration: channel count, sample rate, and codec
change their relationship. Both checks are needed. Invalid or empty audio and
exceeded budgets become HTTP 400 responses; inference failures become HTTP 500.

## Transcription and projection

The [backend](../../packages/elide-stt/src/elide_stt/inference/backends/faster_whisper.py)
materializes faster-whisper's segment iterator before projection. It returns
segment text and, when enabled, per-word text, probability, and timings. Overall
detected language comes from the transcription result, falling back to the
request hint when necessary.

Library timestamps are floating-point seconds. Projection rounds them to
integer milliseconds and clamps negative values to zero. Entries with missing
or inverted timings are dropped; zero-duration spans remain valid. Word
probability becomes confidence. The current backend does not synthesize a
segment-level confidence.

## Optional diarization

Diarization is off by default. Enabling `ELIDE_BENTO_STT_DIARIZE` loads a second
pipeline, defaulting to `pyannote/speaker-diarization-community-1`, and requires
`ELIDE_BENTO_STT_HF_TOKEN`. The warmer checks the token before starting model
downloads. Access to the default model requires accepting its upstream terms.
The token is represented as a secret in settings.

Diarization receives the decoded waveform and returns speaker turns. Each
transcription segment receives the speaker with the greatest total temporal
overlap. This is segment-level attribution; the service does not attach a
separate speaker identity to each word. Minimum and maximum speaker hints can
bound the search. A speaker ID is local to the transcript and does not identify
the same person across requests.

## Rust domain conversion and tests

The [wire models](../../packages/elide-core/src/elide_core/stt/v1.py) use
`start_ms`, `end_ms`, `speaker_id`, `model_id`, and `duration_ms`. The
[Rust adapter](../../crates/adapter/elide-bentoml/src/stt/response.rs) converts
millisecond timings into domain time spans and preserves text, word timings,
speaker labels, language, and confidence where available. It rejects inverted
timings and values that would overflow the domain's microsecond representation.

The wire response also carries overall language, model ID, and decoded duration;
the current Rust domain response does not expose those response-level fields.
[Shared fixtures](../../contracts/stt/v1) cover timed words, speakers, omitted
optional fields, zero-duration spans, and empty transcripts. Decoder tests cover
payload and duration bounds. These establish contract behavior; they do not
measure recognition accuracy or diarization quality.
