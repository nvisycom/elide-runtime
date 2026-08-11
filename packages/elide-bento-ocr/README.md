# elide-bento-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/bento/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/bento/actions/workflows/build.yml)

Default OCR inference service for nvisy. Wraps [docTR](https://github.com/mindee/doctr)
behind an HTTP/JSON endpoint, published as `ghcr.io/nvisycom/elide-bento-ocr`.

## Overview

`OcrService` exposes a single `POST /recognize` endpoint that takes a
base64-encoded image and returns a `Page → Block → Line → Word` hierarchy. docTR
natively produces that hierarchy with **word-level geometry** (its detection +
recognition models), which is what redaction needs to mask sub-line spans.
Request/response types come from [`elide_bento_core.ocr.v1`](../elide-bento-core); the
generated contract lives at [`elide_bento_core.ocr.v1`](../elide-bento-core/src/elide_bento_core/ocr/v1.py).
Any service that speaks the contract can replace it (bring-your-own-inference).

BentoML batches concurrent calls, so the HTTP body wraps the list:
`{"requests": [ { "image": "<base64>", "confidenceThreshold": 0.5 } ]}`; the
response is a JSON array of `OcrResponse`.

### Configuration

- `ELIDE_BENTO_MODEL_PATH` — filesystem path to docTR weights. Takes precedence; also
  satisfied by mounting weights at `/models`.
- `ELIDE_BENTO_MODEL_NAME` — the detection + recognition pair, joined as `det+rec`
  (e.g. `db_resnet50+crnn_vgg16_bn`). Defaults to `db_resnet50+crnn_vgg16_bn`.
- `LOG_LEVEL` — logging level (default `INFO`).

```bash
uv sync
uv run bentoml serve elide_bento_ocr.service:OcrService --reload
```

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/bento/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
