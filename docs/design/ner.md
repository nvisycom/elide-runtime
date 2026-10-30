# NER: schema-driven text spans

`elide-ner` extracts entity spans from text. The Rust runtime uses those spans
for recognition and redaction. The service returns model-native labels and
confidence; the consumer owns taxonomy mapping and policy decisions. See the
[shared service architecture](README.md) for contract ownership and lifecycle.

## Choosing the model

### What the service needs

Beyond [the bar every model meets](README.md#what-every-model-must-meet):

- **Labels chosen per request.** The runtime decides which entity types a
  request looks for, and each label can carry a description that steers it
  toward the intended sense. A model with a fixed label set would need one
  deployment, or one retraining, per set.
- **Exact spans in the original text.** Start and end offsets that point at
  the characters the model matched, unnormalized, so the runtime can redact
  exactly those characters. Generated or paraphrased text has no location.
- **Personal data in particular.** Names, addresses, identifiers, and contact
  details are what the runtime redacts, so a model tuned for them is worth more
  than a general entity model with better scores on news text.
- **Short inputs on CPU, in batches.** Calls carry sentences and paragraphs,
  not documents, and concurrent callers are batched; the model has to handle
  that within the service's 4 GiB on CPU.

### Options considered

| Option | Licence | Languages | Verdict |
|---|---|---|---|
| **`fastino/gliner2-privacy-filter-PII-multi`** (GLiNER2, 205M) | Apache-2.0 | en, fr, es, de, it, pt, nl | **Chosen.** Labels and descriptions per request, tuned on 42 personal-data types |
| `urchade/gliner_multi_pii-v1` (GLiNER v1) | Apache-2.0 | en, fr, de, es, pt, it | Same zero-shot idea, older architecture without GLiNER2's schema; lower in the comparison below |
| `openai/privacy-filter` (1.4B) | Apache-2.0 | not stated | About seven times the size for CPU inference, and lower in the comparison below |
| `nvidia/gliner-PII` | NVIDIA Open Model Licence | en | English only, and a bespoke licence to review |
| Fixed-label token classifiers (BERT NER, spaCy) | Various | Various | A fixed label set: every new entity type is a training job |
| [Presidio](https://github.com/microsoft/presidio) | MIT | Various | A framework of patterns plus a fixed-label NER model; the runtime already runs its own patterns before NER |
| General-purpose LLM extraction | Various | Many | Offsets are unreliable and text can be invented; too slow on CPU. The runtime has a separate LLM classification stage for what it is good at |
| Hosted GLiNER2 ([Pioneer](../../crates/adapter/elide-pioneer-gliner)) | Service terms | Same family | Request text leaves the deployment; available as an adapter for operators who accept that |

### Why GLiNER2's privacy model

[GLiNER2](https://github.com/fastino-ai/GLiNER2) takes the extraction schema
with each call, so one deployed model answers whatever labels and descriptions
the runtime sends, and it returns character spans into the input. The
privacy fine-tune adds what a general GLiNER2 lacks: training on personal-data
types across seven languages. Its
[technical report](https://arxiv.org/abs/2605.09973) gives it the best
span-level F1 (0.477) on the SPY benchmark of the four systems compared,
including the GLiNER v1, OpenAI, and NVIDIA models above. At 205M parameters it
fits the CPU budget with room for batching.

That comparison is the model authors' own and has not been reproduced here.

### Known gaps

- **Seven languages, fewer than OCR reads.** The OCR recognizer also covers
  Polish, Czech, Danish, Norwegian, and Finnish. Text from those documents
  reaches a model that was not trained on them; zero-shot extraction may still
  find some entities, but that is untested.
- **Other languages fail silently.** The encoder (`mdeberta-v3-base`) was
  pretrained on about 100 languages, but the fine-tune was not, and a language
  outside the seven returns a successful response with nothing in it. Measured
  on one Chinese sentence holding a name, phone number, email, address,
  organization, and date: the English translation yields all six at 0.92–1.0
  confidence, and the Chinese yields nothing at 0.5, and at 0.02 only one wrong
  span, a name with the following words attached, whether the labels are in
  English or Chinese. Part of the cause is word splitting: GLiNER2 treats each
  character outside ASCII runs as a word, and spans cover at most eight words,
  so a Chinese address cannot be one span. An empty result is indistinguishable
  from text with no personal data; the runtime's deterministic patterns still
  catch formatted values such as phone numbers and emails, but not names or
  addresses.
- **Synthetic training data.** It was trained on 4,910 generated texts, and an
  F1 of 0.477 leaves many spans missed or wrong. The runtime's deterministic
  patterns run first for anything with a checkable format.
- **512 encoder tokens per input.** Longer text is rejected rather than
  truncated, so callers must split documents.
- **No accuracy measured on this project's data.** Unlike OCR, there is no
  local evaluation yet.

### When to revisit

- A GLiNER2 privacy model covering the Slavic and Nordic languages OCR now
  reads, or any deployment that needs those languages.
- A local evaluation, on the deployment's own documents, showing a candidate
  ahead on the sensitive entity types.
- A GPU deployment, which makes larger encoders affordable.

## How the service runs it

The configured default is selected by `ELIDE_BENTO_NER_MODEL` and loaded once at
startup. Requests select labels, not model weights. The adapter exposes entity
extraction only; GLiNER2's classification and structured-record capabilities
are outside this contract.

The [backend](../../packages/elide-ner/src/elide_ner/inference/backends/gliner2.py)
translates entity descriptions and optional thresholds into the library's
schema, calls `batch_extract`, and projects confidence and offsets into the
[Python wire models](../../packages/elide-core/src/elide_core/ner/v1).

### Request and batch flow

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

### Input budgets and rejection

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

### Offsets and the Rust boundary

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

### Scope and tradeoffs

Results depend on the selected labels, descriptions, and model. The service
provides independent spans; it does not establish entity identity across spans
or requests. Deterministic format validation and policy remain runtime concerns.

The character shortcut relies on the configured backend's character-to-token
bound. A new model or tokenizer needs that assumption checked alongside the
precise token budget. Raising token limits alone does not increase a model's
encoder capacity.
