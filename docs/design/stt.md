# STT: speech with word and segment timings

`elide-stt` transcribes audio into text with time spans. The Rust runtime uses
those timings to relate text recognition back to the audio that may need
redaction. Optional speaker labels provide grouping within a transcript. See the
[shared service architecture](README.md) for the HTTP and deployment boundaries.

## Choosing the model

### What the service needs

Beyond [the bar every model meets](README.md#what-every-model-must-meet):

- **Per-word timings.** The runtime silences or bleeps the stretch of audio
  where a sensitive word was spoken, so it needs a start and end for each word,
  not only for each segment.
- **Many languages, detected rather than declared.** Calls carry recordings in
  whatever language was spoken, often without a hint.
- **Long clips on CPU.** A call can be an hour of audio, transcribed on the CPU
  the published image runs on, within the service's 900-second timeout.
- **Optionally, who spoke when.** Speaker labels group a transcript; they are
  off by default because they need a second model.

### Options considered

Transcription:

| Option | Licence | Languages | Verdict |
|---|---|---|---|
| **Whisper `large-v3-turbo`** on faster-whisper | MIT | 99 | **Chosen.** Native word timings, CPU-viable through CTranslate2 `int8` |
| Whisper `large-v3` on faster-whisper | Apache-2.0 | 99 | A little more accurate, but its 32-layer decoder (turbo has 4) makes it 2–5× slower |
| WhisperX | BSD-2-Clause | 99 | The same Whisper plus a forced-alignment pass, which faster-whisper's native word timings make unnecessary; it also pins `huggingface-hub<1.0.0`, which would hold this service's Hub stack back |
| NVIDIA Parakeet-TDT-0.6B-v3 | CC-BY-4.0 | 25 European, Russian and Ukrainian included | Far faster on GPU, but needs the NeMo runtime and is GPU-oriented |
| NVIDIA Canary-Qwen-2.5B | CC-BY-4.0 | English | English only |
| Mistral Voxtral Small | Apache-2.0 | en, fr, de, es, it, pt, nl, hi | Needs vLLM and about 55 GB of GPU memory |
| Hosted ([Deepgram](../../crates/adapter/elide-deepgram)) | Service terms | Many | Audio leaves the deployment; available as an adapter for operators who accept that |

Diarization:

| Option | Licence | Verdict |
|---|---|---|
| **`pyannote/speaker-diarization-community-1`** | CC-BY-4.0, gated | **Chosen.** Supersedes 3.1, and is WhisperX's current reference |
| `pyannote/speaker-diarization-3.1` | MIT, gated | The previous release, superseded |

### Why Whisper `large-v3-turbo`

It is not the accuracy leader: roughly ten open models score better on the
[Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard).
Each of those fails a requirement above, by needing a GPU serving stack,
covering fewer languages, or both. Turbo keeps Whisper's 99 languages and
language detection, reports word timings natively, and runs on CPU through
[faster-whisper](https://github.com/SYSTRAN/faster-whisper)'s CTranslate2
backend, for a small accuracy cost against `large-v3`. Diarization is assembled
alongside it rather than through WhisperX, so each stage has its own
configuration and loading boundary.

### Known gaps

- **Not the most accurate transcriber available.** A deliberate trade for CPU
  viability and language coverage; see the leaderboard above.
- **Word timings are estimates.** Whisper derives them from its attention, not
  from an acoustic alignment, so a word's edges can be off by a fraction of a
  second. Redacting audio may need a margin around each word.
- **Speakers per segment, not per word.** A segment with two speakers is
  attributed to whichever speaks longer in it.
- **Diarization needs a gated download.** The pyannote model requires accepting
  its terms and a Hugging Face token, so it cannot be warmed anonymously.
- **No accuracy or speed measured on this project's audio.** Fixtures check the
  contract, not recognition quality or real-time factor.
- **Weights not pinned.** faster-whisper resolves `large-v3-turbo`, and the
  diarization model, to the latest commit of their Hub repositories when the
  warmer runs. A warmer run after the publisher pushes new weights serves those
  weights. OCR's `owner/repo@<sha>` form is the model to follow.

### When to revisit

- A GPU deployment: Parakeet becomes the strongest candidate for European
  languages, at the cost of carrying NeMo.
- Redaction needing tighter word edges than Whisper's timings give, which would
  bring back a forced-alignment stage.
- A local evaluation on the deployment's own recordings.

## How the service runs it

The default transcription model is loaded once at startup.
`ELIDE_BENTO_STT_MODEL` selects it: a faster-whisper size, a CTranslate2 repo
id on the Hub, or a local directory of converted weights. Device defaults to CPU,
with `int8` compute; CUDA defaults to `float16`. An explicit compute type
can override that choice. Word timestamps default to enabled.

### Request and bounded decoding

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

### Transcription and projection

The [backend](../../packages/elide-stt/src/elide_stt/inference/backends/faster_whisper.py)
materializes faster-whisper's segment iterator before projection. It returns
segment text and, when enabled, per-word text, probability, and timings. Overall
detected language comes from the transcription result, falling back to the
request hint when necessary.

The language hint is BCP-47 on the wire, and the runtime fills it from the
call's primary language, so a regional tag such as `en-US` is ordinary input.
Whisper accepts only its own bare codes and raises on anything else, so the
backend passes the primary subtag (`en`). A language Whisper does not know is
dropped and the clip is auto-detected, as if no hint had been sent.

Library timestamps are floating-point seconds. Projection rounds them to
integer milliseconds and clamps negative values to zero. Entries with missing
or inverted timings are dropped; zero-duration spans remain valid. Word
probability becomes confidence. The current backend does not synthesize a
segment-level confidence.

### Optional diarization

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

### Rust domain conversion and tests

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
