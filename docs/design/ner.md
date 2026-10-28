# NER: schema-driven text spans

`elide-ner` extracts entity spans from text. The Rust runtime uses those spans
for recognition and redaction. The service returns model-native labels and
confidence; the consumer owns taxonomy mapping and policy decisions. See the
[shared service architecture](README.md) for contract ownership and lifecycle.

## Model and extraction boundary

The backend uses [GLiNER2](https://github.com/fastino-ai/GLiNER2), whose extraction
schema lets the caller name entity types and supply descriptions. This fits the
runtime's per-request label selection: one deployed model can answer different
entity schemas without a separate service for each label set.

The configured default is `fastino/gliner2-privacy-filter-PII-multi`, selected by
`ELIDE_BENTO_NER_MODEL`. It is loaded once at startup. Requests select labels,
not model weights. The adapter exposes entity extraction only; GLiNER2's
classification and structured-record capabilities are outside this contract.

The [backend](../../packages/elide-ner/src/elide_ner/inference/backends/gliner2.py)
translates entity descriptions and optional thresholds into the library's
schema, calls `batch_extract`, and projects confidence and offsets into the
[Python wire models](../../packages/elide-core/src/elide_core/ner/v1).

## Request and batch flow

`POST /recognize` takes a list under `requests`. Each item contains nonempty
`text`, a `schema` with at least one unique entity label, and an optional default
`threshold`. An entity's own threshold overrides the request threshold.

BentoML can combine concurrent callers into a dispatch window of up to 32
items. The [pipeline](../../packages/elide-ner/src/elide_ner/inference/pipeline.py)
then groups items by serialized schema and request threshold. Each group becomes
one model extraction, and responses are restored to original request order.
Different schemas therefore share dispatch capacity without being mixed in one
model call.

The dispatcher allows 15 seconds of queue latency; the service request timeout
is 60 seconds. Increasing batch size changes peak model memory as well as
throughput. The service declares 4 GiB of memory to leave room beyond the model
weights for extraction activations; that budget and the dispatch window should
be evaluated together.

## Input budgets and rejection

Extraction cost grows with both text length and schema content. The service
bounds both dimensions:

- The default text budget is 512 encoder tokens. Short strings skip the
  character scan. Longer strings are scanned incrementally for a dense-character
  rejection bound; candidates that pass are tokenized for the precise check.
  The measured count is reused in response token usage.
- The default schema budget is 32 KiB of serialized UTF-8 JSON. Descriptions
  and labels both count. Counting entity labels alone would leave a single
  large description unbounded.

Schema size is checked while parsing each caller, before adaptive batching,
and checked again at the inference boundary. Token validation currently runs
inside the dispatched pipeline: an overlong text can reject the shared batch.
Per-caller token preparation before dispatch remains an isolation improvement.
An inference failure also fails that dispatched call rather than returning
partial results.

## Offsets and the Rust boundary

Wire `start` and `end` count Unicode code points in the original text, with an
inclusive start and exclusive end. Text is not normalized. For `Łódź Ada`, the
span for `Ada` is `5..8` on the wire and `8..11` in UTF-8 bytes.

The [Rust adapter](../../crates/adapter/elide-bentoml/src/ner/response.rs) builds
an index-to-byte mapping, checks bounds and matched text, and constructs domain
spans. The adapter requires explicit labels. Missing labels fail locally;
an explicitly empty label list returns no work without an HTTP request.

Responses carry `model_id` and optional encoder `tokens`. The current Rust
domain conversion retains the spans and leaves this metadata in the wire layer.
[Shared fixtures](../../contracts/ner/v1) cover ASCII, accents, CJK, emoji, and
combining characters, alongside tests for request ordering and invalid spans.

## Scope and tradeoffs

Results depend on the selected labels, descriptions, and model. The service
provides independent spans; it does not establish entity identity across spans
or requests. Deterministic format validation and policy remain runtime concerns.

The character shortcut relies on the configured backend's character-to-token
bound. A new model or tokenizer needs that assumption checked alongside the
precise token budget. Raising token limits alone does not increase a model's
encoder capacity.
