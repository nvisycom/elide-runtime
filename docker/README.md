# Docker

Compose configuration for the three inference services: NER, OCR, and
speech-to-text. All three are CPU-only.

## Images carry no model weights

The published images are weightless. Each service has a one-shot `*-init`
partner that warms its cache and exits; the service waits on that completing,
so it never starts against an empty cache.

One warmer per service, each running that service's own image, because the
three libraries are not interchangeable and no single image carries all of
them — `elide-stt` has no `onnxtr`, `elide-ocr` has no `faster_whisper`. A
shared warmer would fail on whichever model its image could not resolve.
Warming in the image that later reads the cache also guarantees the same
library version wrote it.

This is deliberate, for three reasons:

**One default model cannot be redistributed.** The diarization model
([`pyannote/speaker-diarization-community-1`](https://huggingface.co/pyannote/speaker-diarization-community-1))
is *gated* on the Hugging Face Hub: downloading it requires accepting its terms
on an account. An image containing it would bypass a consent gate its author
set deliberately, whatever its CC-BY-4.0 licence otherwise permits.

**Baked weights are a default most deployments discard.** Every service reads
its model id from the environment, so an operator pointing at different weights
would have paid for ~3 GB of images they never load.

**Offline becomes true rather than aspirational.** With the cache warm, every
service runs with `HF_HUB_OFFLINE=1` and resolves from disk, so no request-time
traffic reaches the Hub. On a cold cache that flag cannot work at all — the
service has nothing to load and no way to fetch it.

Two cache volumes: `hf_cache` for NER and STT, and `ocr_cache` for OCR, which
holds both OnnxTR's own cache (built-in architectures, hash-checked release
assets) and a Hugging Face cache (Hub models at a pinned commit), so the OCR
service also runs with `HF_HUB_OFFLINE=1`. The services mount them read-only —
only the init containers write.

## Quick start

```bash
cp .env.example .env    # optional: every value is already the default
docker compose up -d
```

The first `up` downloads several gigabytes of weights, so the three `*-init`
containers run for a while before anything else starts. They are idempotent — a
later `up` re-resolves from the volumes and exits immediately.

The services are then on the host at:

| Service | Port | Endpoint |
| :-- | :-- | :-- |
| NER | 3100 | `POST /recognize` |
| OCR | 3101 | `POST /recognize` |
| STT | 3102 | `POST /transcribe` |

Host ports start at 3100 rather than 3000 because 3000 is a common default for
a local web dev server — binding there means a request to `localhost:3000`
silently reaches that instead of the NER service. Inside the compose network
every service still listens on 3000, which is what a server's engine
configuration expects.

Each also serves `/healthz` (liveness) and `/readyz` (readiness, which waits for
the model to load). Compose gates on `/readyz`.

### When a container is unhealthy

The healthcheck runs `python -m elide_core.readyz` and, when it fails, exits
with a one-line reason that Docker keeps:

```console
$ docker inspect --format '{{json .State.Health}}' elide-ner | jq -r '.Log[-1].Output'
loading: accepted the connection but did not answer in 4s
```

Two reasons, needing opposite responses:

- **`loading: ...`** — the service is up but not serving yet. BentoML binds the
  port before the model finishes loading, so this is the normal state during
  startup. Wait; `start_period` allows 60s, against a measured 5–11s.
- **`down: ...`** — nothing is listening. The process died or is crash-looping,
  so read `docker logs <container>`.

Both look identical in `docker ps`, which shows only `unhealthy`.

The request shapes differ: NER and OCR are batching endpoints and take a list,
while STT is not and takes a single object.

```jsonc
// NER, OCR
{ "requests": [ { /* ... */ } ] }
// STT
{ "request": { /* ... */ } }
```

## Pointing a server at the stack

Inside the compose network every service listens on **3000**, reachable as
`ner`, `ocr`, and `stt` — the hostnames and port the server's engine
configuration expects, so no translation is needed:

```toml
[[recognizers.ner]]
kind = "bento"
base_url = "http://ner:3000"

[[enrichers.ocr]]
kind = "bento"
base_url = "http://ocr:3000"

[[enrichers.stt]]
kind = "bento"
base_url = "http://stt:3000"
```

Join the server to the `elide` network, or run both stacks under one compose
project. The host ports in `.env` matter only for reaching a service from
outside the network.

## Development

To run images built from the working tree instead of published ones:

```bash
make services-image      # bentoml build --containerize, all three
docker compose -f compose.yaml -f compose.dev.yaml up -d
```

BentoML builds each image from its `service.py` and `bentofile.yaml`, not a
Dockerfile, so these images cannot be produced by `docker build` and the
override has no `build:` stanza. The override changes only the image names,
log level, and pull policy — ports, caches, the init gates, and the
healthchecks all come from `compose.yaml`, so a fix there applies to both.

## Diarization

Off by default. The model is gated, so enabling it takes three steps:

1. Accept the terms at
   [`pyannote/speaker-diarization-community-1`](https://huggingface.co/pyannote/speaker-diarization-community-1).
2. Create a token on that same account.
3. Set `ELIDE_STT_DIARIZE=true` and `ELIDE_STT_HF_TOKEN=...` in `.env`.

`stt-init` fails with that instruction if the flag is set and the token is
missing, rather than letting the STT service fail its healthcheck later with a
401 buried in a traceback.

## Configuration

See [`.env.example`](.env.example); every value there is the compose default, so
an unedited copy behaves the same as no `.env` at all.

**Model licences are the operator's responsibility.** The defaults are
Apache-2.0 (NER, OCR) and MIT (STT).

> The `large-v3-turbo` STT alias currently resolves to
> `dropbox-dash/faster-whisper-large-v3-turbo`, having been transferred from
> `mobiuslabsgmbh`. Pin the repo id directly in `ELIDE_STT_MODEL` if you need
> that resolution to stay fixed.

## Publishing

[`.github/workflows/docker.yml`](../.github/workflows/docker.yml) builds all
three on every pull request that touches the services, scans each with Trivy,
and publishes to `ghcr.io/nvisycom/elide-{ner,ocr,stt}` on a `v*` tag — `:1.2.3`
plus a moving `:latest`. Pin the version in production.

Each tag is a multi-architecture manifest covering **`linux/amd64`** and
**`linux/arm64`**, so `docker pull` resolves to the host's architecture with no
platform flag.

Both are built on native runners rather than under emulation. That is not a
preference: an emulated PyTorch import is slow enough that the service never
finishes loading its model, so an emulated image would be unusable even where
Docker accepts it.

The per-architecture images are also published, suffixed `-amd64` and
`-arm64`, which is what the manifest points at. Pull one directly only to
reproduce a specific architecture — a deployment should use the plain tag.
