# nvisy-ner

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/inference-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/inference-build.yml)

Self-hosted, schema-driven NER/PII inference service for nvisy, backed by
[GLiNER2](https://github.com/fastino-ai/GLiNER2) behind an HTTP/JSON endpoint,
published as `ghcr.io/nvisy/inference-ner`.

## Overview

`NerService` exposes a single `POST /recognize` endpoint. Each request carries a
**schema** describing what to extract from the text — any combination of:

- **entities** — zero-shot spans for the labels you ask for (`person`, `email`,
  `iban`, …), optionally steered with a description;
- **classifications** — single- or multi-label text classification tasks;
- **structures** — named records of fields (each field can carry a regex).

The response returns each group's results, with character offsets and confidence
scores. Request/response types come from [`nvisy_core.ner.v1`](../nvisy-core);
the generated contract lives at
[`nvisy_core.ner.v1`](../nvisy-core/src/nvisy_core/ner/v1).

Labels are **model-native** — the service returns exactly what the model emits
and does not map onto a shared taxonomy; that mapping is the consumer's job (the
nvisy runtime owns it).

BentoML batches concurrent calls, so the HTTP body wraps the list:

```json
{"requests": [
  {"text": "Ada Lovelace, ada@example.com",
   "schema": {"entities": [{"label": "person"}, {"label": "email"}]},
   "threshold": 0.5}
]}
```

The response is a JSON array of `NerResponse`. Within a batch, requests sharing
an identical schema run as one batched extraction.

### Self-hosted and offline by design

A single GLiNER2 model serves the deployment. Weights are baked into the image
and the service runs with the Hugging Face Hub **offline** (`HF_HUB_OFFLINE=1`),
so text never leaves the operator's network at request time. The service logs
request ids and counts only — never input text or extracted spans. Inputs over
the model's token limit are **rejected** (not silently truncated), so PII past
the limit is never missed.

### Configuration

- `NVISY_NER_MODEL` — the GLiNER2 model id (or local path). Defaults to
  `fastino/gliner2-privacy-filter-PII-multi` (Apache-2.0, SOTA PII span-F1).
- `NVISY_NER_MAX_TOKENS` — reject inputs longer than this (default `512`, the
  model's encoder limit).
- `NVISY_NER_QUANTIZE` — load fp16 (`true`/`false`), roughly halving memory.
- `NVISY_NER_COMPILE` — `torch.compile` the model.
- `LOG_LEVEL` — logging level (default `INFO`).

> **Model licenses are the operator's responsibility.** The default is
> Apache-2.0; an operator-supplied `NVISY_NER_MODEL` is on the operator.

> **Resources.** F32 weights are ~1.2 GB resident; the service defaults to
> `cpu: 2`, `memory: 2Gi`, `max_batch_size: 16`. Set `NVISY_NER_QUANTIZE` to halve
> memory; tune batch/latency from production metrics.

```bash
uv sync
NVISY_NER_MODEL=fastino/gliner2-privacy-filter-PII-multi \
    uv run bentoml serve nvisy_ner.service:NerService --reload
```

The default test suite fakes the model (no weight downloads in CI). A separate
set of `real`-marked tests exercises the engine against the real GLiNER2 model
end to end — excluded by default, run on demand (and by the opt-in `real-models`
CI job):

```bash
uv run pytest -m real          # downloads the GLiNER2 model
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
