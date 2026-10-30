# elide-stt

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Speech-to-text inference service for the elide toolkit, implementing the
`elide_core.stt.v1` wire contract over BentoML. This is the service the
Rust client's `BentoStt` backend targets.

The pipeline is [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
for transcription — including native word-level timings — with optional
[pyannote](https://github.com/pyannote/pyannote-audio) diarization for speaker
labels.

WhisperX bundles the same pieces and is the obvious choice, but what it adds
is a forced-alignment pass, and faster-whisper already reports word timings
natively. It also pins `huggingface-hub<1.0.0`, which would hold this service's
whole Hub stack back for nothing it uses.

## Endpoint

`POST /transcribe` — base64 audio in, segments out:

Unlike the NER and OCR services this endpoint is **not** batching, so the body
wraps a single object under `request` rather than a list under `requests`:

```jsonc
// request
{ "request": { "audio": "<base64>", "filename": "call.wav", "language": "en" } }

// response
{
  "segments": [
    { "start_ms": 0, "end_ms": 2400, "text": "Hello there.",
      "speaker_id": "SPEAKER_00", "language": "en", "confidence": 0.94,
      "words": [{ "start_ms": 0, "end_ms": 480, "text": "Hello", "confidence": 0.97 }] }
  ],
  "model_id": "large-v3-turbo", "language": "en", "duration_ms": 2400
}
```

Timings are **integer milliseconds**. faster-whisper works in float seconds;
the conversion happens once, at the engine boundary.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `ELIDE_BENTO_STT_MODEL` | `large-v3-turbo` | faster-whisper size, Hub repo id, or local weights directory |
| `ELIDE_BENTO_STT_DEVICE` | `cpu` | `cpu` or `cuda` |
| `ELIDE_BENTO_STT_COMPUTE_TYPE` | `int8` (cpu) / `float16` (cuda) | faster-whisper precision |
| `ELIDE_BENTO_STT_WORD_TIMESTAMPS` | on | per-word timings (native to faster-whisper) |
| `ELIDE_BENTO_STT_DIARIZE` | off | pyannote speaker labels |
| `ELIDE_BENTO_STT_HF_TOKEN` | — | required when diarizing (gated model) |
| `ELIDE_BENTO_STT_DIARIZE_MODEL` | `pyannote/speaker-diarization-community-1` | diarization pipeline |
| `ELIDE_BENTO_STT_MIN_SPEAKERS` / `_MAX_SPEAKERS` | — | bound the speaker search |
| `ELIDE_BENTO_STT_MAX_DURATION_SECONDS` | `3600` | reject longer clips |
| `ELIDE_BENTO_STT_MAX_BYTES` | `512 MiB` | reject larger request payloads |

## Why `large-v3-turbo`

MIT-licensed, 99 languages, native word timestamps, and CPU-viable, for a small
accuracy cost against `large-v3` and against the leaderboard's leaders. The
options considered, and what would reopen the choice, are in the
[STT design](../../docs/design/stt.md#choosing-the-model).

## Diarization

Off by default, deliberately: it loads a second model, roughly doubles
latency, and the pipeline is **gated on Hugging Face**. To enable it, accept
the model's terms on its HF page, then set `ELIDE_BENTO_STT_DIARIZE=1` and
`ELIDE_BENTO_STT_HF_TOKEN`.

The default pipeline `pyannote/speaker-diarization-community-1` is CC-BY-4.0.
Note this differs from the Apache-2.0 weights the other services default to —
if your deployment has a licence policy, check it before enabling.

`speaker_id` is the engine's own label (`SPEAKER_00`, …). It groups segments
*within one transcript*; it is not a stable identity across calls.

### Only point `ELIDE_BENTO_STT_DIARIZE_MODEL` at a repo you trust

pyannote loads its pipeline with `weights_only=False` and imports a module
name read out of the checkpoint itself, so **loading a checkpoint is running
its author's code**. `lightning` carries a matching advisory
([PYSEC-2026-3624](https://osv.dev/vulnerability/PYSEC-2026-3624)), fixed
upstream but unreleased as of 2.6.5.

In the default configuration this is not reachable from a request: the
checkpoint loads once at startup from a fixed, gated Hugging Face repo, and
the `/transcribe` handler only ever sees the caller's audio bytes and
language hint — submitted audio never becomes a checkpoint. Repointing this
variable at an untrusted repo is what would turn it into a code-execution
path, so treat it as you would any other "run this binary" setting.

## Run locally

```bash
make sync          # once: the tooling, plus each service's own .venv
make serve-stt
```

or without make, from the repository root (the service resolves its exported
requirements against the working directory, so `make requirements` first):

```bash
uv run --project packages/elide-stt bentoml serve elide_stt.service:SttService --reload
```

Either way the model is downloaded on first start. To run the published
image instead, with its cache warmed by a one-shot init container, see
[`docker/`](../../docker/README.md).

The default test suite fakes the model. The `real`-marked tests transcribe a
pinned LibriSpeech clip with `tiny`, with a regional language hint, and load
again with the Hub offline. They are excluded by default; CI's `real-models`
job runs them when this service or `elide-core` changes:

```bash
uv run --project packages/elide-stt pytest packages/elide-stt -m real   # downloads tiny and the clip
```

## Versioning

The Python models in [`elide-core`](../elide-core) own the versioned HTTP
contract. The [contract generator](../../scripts/gen_contracts.py) exports
schemas and shared fixtures and uses Typify to generate the Rust wire types for
[`elide-bentoml`](../../crates/adapter/elide-bentoml). CI checks for drift and
tests both languages against the shared fixtures. Change the Python models and
regenerate the artifacts rather than editing generated Rust types; see the
[STT v1 contract](../../contracts/stt/v1).

## Project

- **Changelog**: [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
