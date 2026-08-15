# elide-bento-vl

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-bento/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-bento/actions/workflows/build.yml)

Vision-language OCR verification service for nvisy. Wraps
[PaddleOCR-VL](https://github.com/PaddlePaddle/PaddleOCR) behind an HTTP/JSON
endpoint, published as `ghcr.io/nvisycom/elide-bento-vl`.

## Overview

`OcrVlService` exposes a single `POST /recognize` endpoint that takes a
base64-encoded image and returns **block-level regions** — each with text, a
layout `BlockKind`, a bounding box, and a reading-order index. A VLM reads the
whole page with high text accuracy; this service reports that reading. The
runtime **reconciles** it with a detection-OCR result (geometry from
[`elide-bento-ocr`](../elide-bento-ocr), text refined by the VLM). Request/response types
come from [`elide_bento_core.ocrvl.v1`](../elide-bento-core).

This is a **GPU service** — PaddleOCR-VL is a ~0.9B vision-language model. It
runs on CPU but slowly; set `ELIDE_BENTO_DEVICE=cpu` for CPU-only deployments. It is
**opt-in**: deployments that don't need VL verification simply don't run it.

BentoML batches concurrent calls, so the HTTP body wraps the list:
`{"requests": [ { "image": "<base64>" } ]}`; the response is a JSON array of
`VlResponse`.

### Configuration

- `ELIDE_BENTO_MODEL_PATH` — filesystem path to model weights. Takes precedence; also
  satisfied by mounting weights at `/models`.
- `ELIDE_BENTO_MODEL_NAME` — model id to load. Defaults to `PaddlePaddle/PaddleOCR-VL`.
- `ELIDE_BENTO_DEVICE` — `gpu` (default) or `cpu`.
- `LOG_LEVEL` — logging level (default `INFO`).

```bash
uv sync
uv run bentoml serve elide_bento_vl.service:OcrVlService --reload
```

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-bento/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
