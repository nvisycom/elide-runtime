# NER — schema-driven extraction with GLiNER2

`nvisy-ner` extracts PII and named entities from text and is the source of the
spans the runtime redacts. It is a **schema-driven** service backed by a single
[GLiNER2](https://github.com/fastino-ai/GLiNER2) model: each request carries a
schema (entities, classification tasks, structured records) and the service runs
one batched extraction.

## Why GLiNER2

GLiNER2 is the state of the art for self-hosted PII extraction — it tops
span-level F1 on the SPY benchmark over OpenAI's Privacy Filter and the earlier
GLiNER PII models ([arxiv 2605.09973](https://arxiv.org/abs/2605.09973)) — while
running on CPU at ~0.3B parameters. Its schema interface covers three tasks in
one model (entity extraction, text classification, structured parsing), so one
engine serves the whole contract.

The default model is `fastino/gliner2-privacy-filter-PII-multi`: Apache-2.0,
multilingual (7 languages), 42 PII labels, `microsoft/mdeberta-v3-base` encoder.

## Model-native labels

The service returns each span's **raw model label** (`person`, `email`, `iban`,
…) together with the `modelId` that produced it. It does not map labels onto a
shared taxonomy — that is the **consumer's** job (the runtime's `nvisy-ontology`
owns the map, keyed by `modelId`). The contract lives in
[`nvisy_core.ner.v1`](../../packages/nvisy-core/src/nvisy_core/ner/v1.py).

## The schema

A request's schema composes three optional groups, mirroring GLiNER2's own
`Schema`:

- **entities** — zero-shot spans for the requested labels; a per-label
  description steers the model. Returns character offsets (`start`/`end`) and a
  confidence score — exactly what the redaction pipeline needs to mask a span.
- **classifications** — single- or multi-label text classification tasks.
- **structures** — named records of fields; a field can carry an enum of choices
  and a regex pattern (compiled to a GLiNER2 `RegexValidator`).

The engine ([`engine.py`](../../packages/nvisy-ner/src/nvisy_ner/engine.py))
translates the wire schema into a `gliner2.Schema`, calls `batch_extract`, and
projects the result back into the typed response (GLiNER2's `confidence` becomes
`score`).

## Single model, self-hosted

One GLiNER2 model serves the deployment, named by `NVISY_NER_MODEL`. There is no
whitelist and no per-request model selection — a self-hosted appliance serves one
taxonomy per deployment; an operator who needs a different model sets the env var
and redeploys.

Self-hosting is the point, so the service is built to keep data on-box:

- **Offline.** Weights are baked into the image; the service runs with
  `HF_HUB_OFFLINE=1` / `TRANSFORMERS_OFFLINE=1`, so it never reaches the Hub at
  request time and never uses GLiNER2's hosted API path.
- **No payload logging.** Logs carry request ids and counts, never input text or
  spans.
- **Reject, don't truncate.** The encoder caps at 512 tokens and silently
  truncates above it — which would drop PII in the tail unseen. The service
  rejects over-length input (`NVISY_NER_MAX_TOKENS`) instead.

## What it is *not*

- **Not a pattern matcher.** Structured PII with strong syntactic signals
  (credit-card Luhn, key formats) is also caught by deterministic patterns in the
  runtime; a field regex narrows extraction but the model is not a validator.
- **Not coreference / linking.** Spans are independent — no entity IDs, no
  cross-span clustering.
- **Not for non-text modalities.** Visual/biometric entities (faces, signatures,
  barcodes) belong to the OCR/CV path.

A formal SOTA review is tracked at
[issue #20](https://github.com/nvisycom/runtime/issues/296).
