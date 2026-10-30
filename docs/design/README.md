# Inference service design

The inference services turn source content into observations the Rust runtime
can locate: [NER](ner.md) produces text spans, [OCR](ocr.md) produces image
regions, and [STT](stt.md) produces timed speech segments. The runtime owns policy,
entity interpretation, and redaction; each service owns model execution and
conversion into a versioned HTTP contract.

Each task document is first a guide to choosing its model: what the task needs
from one, the options considered, why the current default was picked, where it
falls short, and what would reopen the choice. The second half describes how
the service runs the model. Endpoint examples live in the package READMEs;
deployment instructions live in [the Docker documentation](../../docker/README.md).

## What every model must meet

These requirements apply to every default model, and a replacement has to meet
them before its accuracy is worth comparing. Each task document adds its own
requirements on top of these.

- **Self-hosted, and offline once warmed.** Request content is the sensitive
  data being redacted, so the default never sends it to a third party. A warmed
  cache must serve the model with the network unavailable. Hosted engines exist
  for operators who accept that trade, as separate Rust adapters
  ([Textract](../../crates/adapter/elide-aws-textract),
  [Document AI](../../crates/adapter/elide-google-document-ai),
  [Deepgram](../../crates/adapter/elide-deepgram),
  [Pioneer](../../crates/adapter/elide-pioneer-gliner)), never as these services.
- **Licensed for commercial use.** Apache-2.0, MIT, or CC-BY with attribution. A
  licence that limits commercial use, by revenue or otherwise, is disqualifying
  however good the model is.
- **Viable on CPU.** The published images are CPU-only, and a deployment must
  not need a GPU to run the defaults. GPU support is an option, not a premise.
- **Pinned weights.** A deployment must load the same weights tomorrow as today:
  a hash-checked asset, a full commit, or a library version that pins both. A
  branch or a moving tag does not qualify.
- **A runtime the image can carry.** One Python process alongside BentoML. A
  model that needs its own serving stack, such as vLLM or NeMo, has to justify
  that stack's size and operations as well as its accuracy.
- **Output the runtime can locate.** The runtime redacts where the model says
  the data is: character offsets, pixel boxes, or millisecond timings. A model
  that returns plausible text without a faithful location, or text that is not
  in the source, is unusable regardless of its accuracy.
- **The deployment's languages, across all three services.** The default
  coverage is European Latin-script languages. A document passes through OCR
  and then NER, so a language one service reads and the next was not trained on
  is a gap even when each model is fine on its own.

Accuracy is then measured on what redaction needs. A model that reads 95% of
words but misses the names is worse here than one that reads 93% and gets them
all, so each evaluation scores the sensitive tokens separately.

## Contract ownership

Pydantic models in [`elide-core`](../../packages/elide-core) define the wire
contract. JSON fields use snake_case. The consistently named `NerInput/NerOutput`,
`OcrInput/OcrOutput`, and `SttInput/SttOutput` describe HTTP envelopes:

| Service | Request | Response | Coordinate system |
| --- | --- | --- | --- |
| NER | `{"requests": [...]}` | Response array | Unicode code-point offsets |
| OCR | `{"requests": [...]}` | Response array | Image pixels |
| STT | `{"request": {...}}` | Response object | Integer milliseconds |

The [contract generator](../../scripts/gen_contracts.py) exports schemas and
shared fixtures and invokes pinned Typify to generate Rust wire types. The
[contract snapshots](../../contracts) and generated types are tracked in Git.
CI checks regeneration and runs Python and Rust tests against the same fixtures.

The Rust adapters translate wire values into domain types. This is where text
indices become byte ranges, OCR hierarchies become flat regions, and speech
timestamps become domain time spans. Schema generation does not replace these
semantic conversions or their tests.

## Package boundaries

Each service follows the same layout:

```text
elide_<task>/
├── service.py              # BentoML entry point and HTTP error mapping
├── warm.py                 # Standalone model-cache preparation
├── inference/
│   ├── pipeline.py         # Input preparation and inference orchestration
│   ├── validation.py       # Input checks and shared validation errors
│   └── backends/           # Factory, Protocol, and model implementations
└── runtime/
    ├── config.py           # Validated, immutable environment settings
    ├── metrics.py          # Prometheus instruments
    └── image.py            # BentoML build configuration
```

Package initialization is lightweight. Backend factories select one
implementation and import model libraries when constructing the engine. The
service reads a cached settings object and passes it into the backend; request
handling uses the same configuration snapshot. Warmers are separate processes
and read their own settings.

## Execution and model lifecycle

Handlers are synchronous because inference blocks on CPU or GPU work. BentoML
runs them in its managed thread pool. NER uses adaptive batching and then groups
compatible schemas; OCR uses adaptive dispatch but processes its images
sequentially; STT handles one clip per call. Each task's design explains the
resulting latency and failure boundaries.

The supplied Compose deployment runs a one-shot warmer before each service. It
downloads model weights into a writable cache and hands ownership to the service
user. Services mount those caches read-only. Weights are prepared during
initialization rather than baked into the published images. Every service then
serves with `HF_HUB_OFFLINE=1`, which only its warmer turns off, so once warm a
service loads its models from disk and makes no request to the Hub.

`/readyz` gates traffic after model initialization; `/healthz` reports process
liveness. Standalone warmers configure logging through `elide_core.warm`;
service loggers use BentoML's handlers. Metrics report work size and rejection
reasons, while request IDs correlate service logs with the caller.

## Changing an implementation

A new backend implements its task's Protocol and joins the backend registry.
Its loading, output projection, and cache preparation must agree. Changing the
registry alone does not add a warmer for a different model family.

Changing the wire starts with Python models, followed by regeneration and
cross-language tests. Changes to offset units, geometry, or timing need
behavioral fixtures even if the JSON field types stay identical. Changes to
batch windows and input limits also require checking memory use and queueing
behavior; they are part of the service's capacity design.
