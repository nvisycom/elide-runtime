# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/py-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/py-build.yml)

Default OCR inference service for nvisy. Wraps [docTR](https://github.com/mindee/doctr)
behind an HTTP/JSON endpoint, published as `ghcr.io/nvisy/inference-ocr`.

## Overview

`OcrService` exposes a single `POST /recognize` endpoint that takes a
base64-encoded image and returns a `Page → Block → Line → Word` hierarchy. docTR
natively produces that hierarchy with **word-level geometry** (its detection +
recognition models), which is what redaction needs to mask sub-line spans.
Request/response types come from [`nvisy_core.ocr.v1`](../nvisy-core); the
generated contract lives at [`nvisy_core.ocr.v1`](../nvisy-core/src/nvisy_core/ocr/v1.py).
Any service that speaks the contract can replace it (bring-your-own-inference).

BentoML batches concurrent calls, so the HTTP body wraps the list:
`{"requests": [ { "image": "<base64>", "confidenceThreshold": 0.5 } ]}`; the
response is a JSON array of `OcrResponse`.

### Configuration

- `NVISY_MODEL_PATH` — filesystem path to docTR weights. Takes precedence; also
  satisfied by mounting weights at `/models`.
- `NVISY_MODEL_NAME` — the detection + recognition pair, joined as `det+rec`
  (e.g. `db_resnet50+crnn_vgg16_bn`). Defaults to `db_resnet50+crnn_vgg16_bn`.
- `LOG_LEVEL` — logging level (default `INFO`).

```bash
uv sync
uv run bentoml serve nvisy_ocr.service:OcrService --reload
```

## Documentation

See [`docs/`](../../docs/) for the generated OpenAPI specs and contract
documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
