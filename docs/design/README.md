# Inference service design

The inference services turn source content into observations the Rust runtime
can locate: [NER](ner.md) produces text spans, [OCR](ocr.md) produces image
regions, and [STT](stt.md) produces timed speech segments. The runtime owns policy,
entity interpretation, and redaction; each service owns model execution and
conversion into a versioned HTTP contract.

These documents describe implemented behavior and the decisions behind it.
Endpoint examples live in the package READMEs; deployment instructions live in
[the Docker documentation](../../docker/README.md).

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
initialization rather than baked into the published images. NER enables Hub
offline settings while serving. Other backends retain their library-specific
loading behavior; a warmed cache alone is not a network-isolation guarantee.

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
