# bento-whisper

Speech-to-text inference service for the elide toolkit, implementing the
`bento_core.stt.v1` wire contract over BentoML. This is the service the
Rust client's `BentoStt` backend targets.

The pipeline is [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
for transcription — including native word-level timings — with optional
[pyannote](https://github.com/pyannote/pyannote-audio) diarization for speaker
labels.

WhisperX bundles the same pieces and is the obvious choice, but it pins
`huggingface-hub<1.0.0`. `bento-gliner2` requires `transformers>=5.15.0`,
whose own resolved version requires `huggingface-hub>=1.5.0` — so the two
ceilings cannot coexist in this workspace's single lock. Assembling the stages
directly avoids that ceiling, and costs little: faster-whisper reports word
timings natively, so the separate forced-alignment pass WhisperX exists for is
not needed.

## Endpoint

`POST /transcribe` — base64 audio in, segments out:

```jsonc
// request
{ "audio": "<base64>", "filename": "call.wav", "language": "en" }

// response
{
  "segments": [
    { "startMs": 0, "endMs": 2400, "text": "Hello there.",
      "speakerId": "SPEAKER_00", "language": "en", "confidence": 0.94,
      "words": [{ "startMs": 0, "endMs": 480, "text": "Hello", "confidence": 0.97 }] }
  ],
  "modelId": "large-v3-turbo", "language": "en", "durationMs": 2400
}
```

Timings are **integer milliseconds**. faster-whisper works in float seconds;
the conversion happens once, at the engine boundary.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `ELIDE_BENTO_MODEL_NAME` | `large-v3-turbo` | faster-whisper model id |
| `ELIDE_BENTO_MODEL_PATH` | `/models` | BYO weights mount, when present |
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

MIT-licensed, 99 languages, native word timestamps, and 2-5x faster than
`large-v3` (its decoder is pruned from 32 layers to 4) for a small accuracy
cost.

It is **not** the WER leader. On the
[Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard)
roughly ten open models score better — but each fails a bar that matters more
here:

| Model | Why not |
|---|---|
| NVIDIA Parakeet-TDT-0.6B-v3 | ~50x faster, but needs the NeMo runtime, is GPU-oriented, and covers 25 European languages rather than 99 |
| NVIDIA Canary-Qwen-2.5B | English-only |
| Mistral Voxtral Small | vLLM, ~55GB VRAM |
| ARK-ASR-3B, MOSS-Transcribe | Preview models from newer labs; licences not independently confirmed |

So this is a deliberate trade of raw accuracy for ecosystem fit, CPU viability
and language coverage — the same bar the NER service sets for its default
("Apache-2.0, CPU-viable, multilingual"). Parakeet is the one worth revisiting
if this service ever moves to GPU.

## Diarization

Off by default, deliberately: it loads a second model, roughly doubles
latency, and the pipeline is **gated on Hugging Face**. To enable it, accept
the model's terms on its HF page, then set `ELIDE_BENTO_STT_DIARIZE=1` and
`ELIDE_BENTO_STT_HF_TOKEN`.

The default pipeline `pyannote/speaker-diarization-community-1` is CC-BY-4.0.
Note this differs from the Apache-2.0 weights the other services default to —
if your deployment has a licence policy, check it before enabling.

`speakerId` is the engine's own label (`SPEAKER_00`, …). It groups segments
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
make serve-whisper
# or: uv run bentoml serve bento_whisper.service:SttService --reload
```
