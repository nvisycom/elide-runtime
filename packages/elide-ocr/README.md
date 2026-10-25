# elide-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Default OCR inference service for nvisy. Wraps [docTR](https://github.com/mindee/doctr)
behind an HTTP/JSON endpoint, published as `ghcr.io/nvisycom/elide-ocr`.

## Overview

`OcrService` exposes a single `POST /recognize` endpoint that takes a
base64-encoded image and returns a `Page → Block → Line → Word` hierarchy. docTR
natively produces that hierarchy with **word-level geometry** (its detection +
recognition models), which is what redaction needs to mask sub-line spans.
Request/response types come from [`elide_core.ocr.v1`](../elide-core); the
generated contract lives at [`elide_core.ocr.v1`](../elide-core/src/elide_core/ocr/v1.py).
Any service that speaks the contract can replace it (bring-your-own-inference).

BentoML batches concurrent calls, so the HTTP body wraps the list:
`{"requests": [ { "image": "<base64>", "confidenceThreshold": 0.5 } ]}`; the
response is a JSON array of `OcrResponse`.

## Configuration

- `ELIDE_BENTO_MODEL_PATH` — filesystem path to docTR weights. Takes precedence; also
  satisfied by mounting weights at `/models`.
- `ELIDE_BENTO_MODEL_NAME` — the detection + recognition pair, joined as `det+rec`
  (e.g. `db_resnet50+crnn_vgg16_bn`). Defaults to `db_resnet50+crnn_vgg16_bn`.
- `LOG_LEVEL` — logging level (default `INFO`).

```bash
uv sync
uv run bentoml serve elide_ocr.service:OcrService --reload
```

## Run locally

```bash
make serve-ocr
```

The model is downloaded on first start. To run the published image instead,
with its cache warmed by a one-shot init container, see
[`docker/`](../../docker/README.md).

## Versioning

Lockstep with the workspace. The wire contract lives in
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
