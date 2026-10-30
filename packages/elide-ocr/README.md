# elide-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Default OCR inference service for nvisy. Wraps [OnnxTR](https://github.com/felixdittrich92/OnnxTR)
([docTR](https://github.com/mindee/doctr)'s models on ONNX Runtime, no PyTorch)
behind an HTTP/JSON endpoint, published as `ghcr.io/nvisycom/elide-ocr`.

## Overview

`OcrService` exposes a single `POST /recognize` endpoint that takes a
base64-encoded image and returns a `Page → Block → Line → Word` hierarchy. OnnxTR
natively produces that hierarchy with **word-level geometry** (its detection +
recognition models), which is what redaction needs to mask sub-line spans.
Request/response types come from [`elide_core.ocr.v1`](../elide-core); the
generated contract lives at [`elide_core.ocr.v1`](../elide-core/src/elide_core/ocr/v1.py).
Any service that speaks the contract can replace it (bring-your-own-inference).

BentoML batches concurrent calls, so the HTTP body wraps the list:
`{"requests": [ { "image": "<base64>", "confidence_threshold": 0.5 } ]}`; the
response is a JSON array of `OcrResponse`.

## Configuration

Each model is a built-in OnnxTR architecture such as `db_resnet50`, or a Hugging Face
Hub model pinned to a full commit, `<owner>/<repo>@<40-hex sha>`; an unpinned Hub name is
rejected at startup.

- `ELIDE_BENTO_OCR_DETECTION` — detection model (default `db_resnet50`).
- `ELIDE_BENTO_OCR_RECOGNITION` — recognition model (default
  `Felix92/onnxtr-parseq-multilingual-v1@7d7329948dbcff02637a3eb9c14ae0c4440c5ed0`, docTR's
  multilingual parseq: twelve European Latin-script languages, no Cyrillic).
- `ELIDE_BENTO_OCR_DEVICE` — `cpu` (default) or `cuda`. `cuda` needs an image built
  with `onnxruntime-gpu` and fails at startup without it rather than falling back.
- `ONNXTR_CACHE_DIR` — where OnnxTR keeps built-in weights (OnnxTR's default is
  `~/.cache/onnxtr`).
- `HF_HOME` — where Hub models are cached (the Hugging Face default, `~/.cache/huggingface`).
- `LOG_LEVEL` — logging level (default `INFO`).

## Run locally

```bash
make sync          # once: the tooling, plus each service's own .venv
make serve-ocr
```

or without make, from the repository root (the service resolves its exported
requirements against the working directory, so `make requirements` first):

```bash
uv run --project packages/elide-ocr bentoml serve elide_ocr.service:OcrService --reload
```

The weights are downloaded on first start. To run the published image instead,
with its cache warmed by a one-shot init container, see
[`docker/`](../../docker/README.md).

The default test suite fakes the predictor. The `real`-marked tests load the
default weights, read accented names off a rendered page, and load again with
the Hub offline. They are excluded by default; CI's `real-models` job runs them
when this service or `elide-core` changes:

```bash
uv run --project packages/elide-ocr pytest packages/elide-ocr -m real   # downloads the default models
```

## Versioning

Versioned with the repository. The wire contract lives in
[`elide-core`](../elide-core), and the Rust client that consumes it,
[`elide-bentoml`](../../crates/adapter/elide-bentoml), is the source of truth
for the format.

## Project

- **Changelog**: [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
