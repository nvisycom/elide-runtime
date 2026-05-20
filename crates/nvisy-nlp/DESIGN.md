# `nvisy-nlp` design

A crate for **offline NLP** in nvisy — primarily NER, but composable
with language detection and tokenization. Inspired by Microsoft
Presidio's `NlpEngine` abstraction but reshaped for Rust realities and
nvisy's actual needs.

## Goals

1. **Composable backends.** A consumer should be able to pick "ONNX
   NER + lingua language detection + HF tokenizer" without one backend
   pretending to do all three.
2. **One trait per concern.** `NerBackend`, `LanguageDetector`,
   `Tokenizer` — each is independently implementable.
3. **Async-native.** NER is the slow operation and is naturally async
   (HTTP-style call patterns even for local ONNX inference on a
   blocking pool). Other operations are sync.
4. **Return ontology types.** No translation layer between `nvisy-nlp`
   and `nvisy-ontology::entity::*`.
5. **No surprise dependencies.** Every dep is audited for maintenance
   status. No zombie crates.
6. **Pure Rust.** No PyO3. No bundled Python interpreter. No CUDA at
   build time (optional at runtime via ORT execution providers).

## Non-goals

- Lemmatization. No maintained Rust lemmatizer exists; nothing
  currently consumes lemmas; defer until a real consumer needs them.
  See [#154](https://github.com/nvisycom/runtime/issues/154).
- Dependency parsing. Doesn't exist in Rust.
- Coreference resolution. Doesn't exist in Rust.
- Entity linking. Doesn't exist in Rust.
- POS tagging beyond what an NER model produces. No maintained Rust
  option.
- LLM-mediated NER. Lives in `nvisy-llm` (future crate, see provider
  split task). `nvisy-nlp` is *offline-only*.
- Model bundling. Models are 100MB-2GB; user provides paths to the
  files they want loaded.
- Model auto-download. Breaks air-gapped deployments. User installs
  models out of band.

## Layout

```
nvisy-nlp/
├── Cargo.toml
├── DESIGN.md           (this file)
├── README.md
└── src/
    ├── lib.rs
    ├── error.rs        — NlpError + From impl to nvisy_core::Error
    ├── artifacts.rs    — NlpArtifacts, Token
    ├── engine.rs       — NlpEngine composite + analyze() orchestration
    ├── ner/
    │   ├── mod.rs      — NerBackend trait
    │   ├── ort.rs      — OrtNerBackend (ONNX + HF tokenizer)
    │   └── noop.rs     — NoopNerBackend
    ├── language/
    │   ├── mod.rs      — LanguageDetector trait
    │   └── lingua.rs   — LinguaLanguageDetector
    └── tokenizer/
        ├── mod.rs      — Tokenizer trait
        ├── unicode.rs  — UnicodeTokenizer (no model)
        └── hf.rs       — HfTokenizer (wraps tokenizers crate)
```

## Trait surface

### `NerBackend`

```rust
#[async_trait]
pub trait NerBackend: Send + Sync {
    async fn recognize(
        &self,
        text: &str,
        language: Option<&LanguageTag>,
    ) -> Result<Entities, NlpError>;

    /// Empty = backend accepts any language.
    fn supported_languages(&self) -> &[LanguageTag] { &[] }

    /// Empty = backend produces any kind it can detect.
    fn supported_kinds(&self) -> &[EntityKind] { &[] }
}
```

Async because the only realistic implementation paths (ONNX
inference, future LLM-backed) need to yield. Synchronous impls wrap
in `async {}` trivially.

`language` is advisory. ONNX models trained on multilingual data
ignore it; monolingual models can validate against
`supported_languages`.

### `LanguageDetector`

```rust
pub trait LanguageDetector: Send + Sync {
    fn detect(&self, text: &str) -> Option<LanguageTag>;
}
```

Sync because lingua's detection is pure CPU and fast. Returns
`Option` because language detection on very short text is unreliable
and "I don't know" is a valid answer.

### `Tokenizer`

```rust
pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<Token>;
}

pub struct Token {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub is_stop: bool,
    pub is_punct: bool,
}
```

`start` / `end` are byte offsets into the original text. `is_stop`
and `is_punct` are determined by the tokenizer impl (consults its
language's stopword list and Unicode category, respectively).

## Data carrier: `NlpArtifacts`

```rust
pub struct NlpArtifacts {
    pub entities: Entities,
    pub language: Option<LanguageTag>,
    pub tokens: Option<Vec<Token>>,
    pub keywords: Option<HashSet<String>>,
}
```

Modeled on Presidio's `NlpArtifacts` but stripped to what's actually
useful. Presidio's fields and their fate here:

| Presidio field | Fate in nvisy-nlp | Reason |
|---|---|---|
| `entities` | Kept as `Entities` | Core output |
| `scores` | Folded into each `Entity` (already has confidence) | Avoid parallel arrays |
| `tokens` (spaCy `Doc`) | Kept as `Vec<Token>` | Plain data, no engine objects |
| `tokens_indices` | Folded into `Token.start` / `end` | Same data, better structure |
| `lemmas` | **Dropped** | No Rust lemmatizer; [#154](https://github.com/nvisycom/runtime/issues/154) |
| `keywords` | Kept as `HashSet<String>` | Derived: tokens minus stopwords/punct |
| `language` | Kept as `Option<LanguageTag>` | Typed (BCP-47) instead of bare string |
| `nlp_engine` | **Dropped** | Backref pattern unnecessary in Rust |

`tokens` and `keywords` are `Option` because not every backend
configuration produces them — a "just NER" pipeline may skip
tokenization entirely.

## Composite: `NlpEngine`

```rust
pub struct NlpEngine {
    ner: Arc<dyn NerBackend>,
    language: Option<Arc<dyn LanguageDetector>>,
    tokenizer: Option<Arc<dyn Tokenizer>>,
    stopwords: Option<HashSet<String>>,
}

impl NlpEngine {
    pub async fn analyze(&self, text: &str) -> Result<NlpArtifacts, NlpError>;
}
```

`analyze()` orchestrates in **Presidio order**:

1. **Detect language** (if a detector is configured). Result is
   passed to NER as a hint.
2. **Recognize entities** via the NER backend.
3. **Tokenize** (if a tokenizer is configured) — produces `tokens`.
4. **Derive keywords** as `tokens.filter(|t| !t.is_stop && !t.is_punct)`
   when both tokens and a stopword list are present.

The detector, tokenizer, and stopword list are all optional. A
minimal engine is just `ner` — `analyze()` returns entities and
nothing else.

## v1 implementations

### `OrtNerBackend`

ONNX-based NER over `ort` + `tokenizers`. Configuration:

```rust
pub struct OrtNerConfig {
    pub model_path: PathBuf,        // .onnx file
    pub tokenizer_path: PathBuf,    // tokenizer.json
    pub label_map: HashMap<String, EntityKind>,
    pub max_sequence_length: usize, // default 512
}
```

User provides the model. Recommended starting points (documented in
README, not bundled):

- `dslim/bert-base-NER` (English, PER/ORG/LOC/MISC)
- `Davlan/bert-base-multilingual-cased-ner-hrl` (10 languages)
- `Jean-Baptiste/roberta-large-ner-english`

Convert with `optimum`:

```bash
optimum-cli export onnx --model dslim/bert-base-NER ./out
```

Inference runs on the tokio blocking pool because ORT is sync.

### `LinguaLanguageDetector`

```rust
pub struct LinguaLanguageDetector {
    inner: lingua::LanguageDetector,
}
```

Construction takes the language allowlist (lingua needs it explicit
for memory efficiency — loading all 75 languages is ~600MB; loading
~10 is ~80MB).

### `UnicodeTokenizer`

Pure `unicode-segmentation`. No model, no deps beyond
`unicode-segmentation` and a builtin stopword list (via `stop-words`
crate). Returns tokens with `is_punct` from Unicode category and
`is_stop` from the configured stopword set.

Use this when you want tokens but don't have an ML model handy.

### `HfTokenizer`

Wraps a `tokenizers::Tokenizer` loaded from `tokenizer.json`. Used
when the tokenization needs to match a specific HF model — e.g., for
alignment with `OrtNerBackend`'s offsets.

```rust
pub struct HfTokenizer {
    inner: tokenizers::Tokenizer,
    stopwords: Option<HashSet<String>>,
}
```

### `NoopNerBackend`

Returns empty `Entities`. For tests that need an `NlpEngine` but
don't care about NER output.

## Errors

```rust
pub enum NlpError {
    /// Failed to load an ONNX model.
    ModelLoad { path: PathBuf, source: ort::Error },
    /// Failed to load or apply a HF tokenizer.
    Tokenizer(String),
    /// Inference itself failed.
    Inference(String),
    /// Backend doesn't support the requested language.
    UnsupportedLanguage(LanguageTag),
    /// Generic catch-all from a backend impl.
    Backend(String),
}
```

`From<NlpError> for nvisy_core::Error` so callers can use `?`.

## Dependencies

All audited for maintenance status before adoption.

| Crate | Purpose | Verdict |
|---|---|---|
| `nvisy-core` | Shared error type | Internal |
| `nvisy-ontology` | `Entity`, `LanguageTag`, `EntityKind` | Internal |
| `async-trait` | Async fn in traits | Standard |
| `ort` 2.x | ONNX Runtime bindings | Production (pykeio) |
| `tokenizers` 0.23 | HF tokenizer for OrtNerBackend, HfTokenizer | Production (HF) |
| `lingua` 1.8 | Language detection | Production |
| `unicode-segmentation` | Tokenizer fallback | Production |
| `stop-words` 0.10 | Stopword lists | Actively maintained |
| `smallvec` | Per-entity vec | Production |
| `thiserror` | Error derive | Standard |
| `tracing` | Observability | Standard |

Six external NLP-relevant deps. No dead crates.

## Deferred to v2 (explicit list)

Each of these has been considered and intentionally deferred. They
should be added only when a concrete consumer materializes.

- **`GlinerBackend`** — zero-shot NER via `gline-rs`. The trait
  absorbs it cleanly; ship when zero-shot is actually requested.
- **Lemmatization** — see [#154](https://github.com/nvisycom/runtime/issues/154).
  Three paths when needed: static lookup table, fork nlprule, PyO3
  to spaCy. Decide at adoption time.
- **`CandleNerBackend`** — pure-Rust escape hatch from ORT's
  dynamic ONNX Runtime libloading. Ship if libloading becomes a
  deployment pain.
- **POS tagging** — only if a maintained Rust crate appears.
- **Embedding-backed recognizers** — `finalfusion` is solid for
  reading; nothing currently consumes embeddings.
- **`whichlang` / `whatlang` backends** — alternative language
  detectors behind feature flags. Add when someone needs a smaller
  binary or wider language coverage than lingua.

## Coupling

`nvisy-nlp` depends on `nvisy-core` and `nvisy-ontology`. Nothing
in nvisy depends on `nvisy-nlp` yet; the first consumer will be
`nvisy-engine::operation::detection::EntityRecognition` after task
#48 migrates it to consume an `Arc<dyn NerBackend>`.

`nvisy-nlp` does **not** depend on:

- `nvisy-pattern` — patterns are deterministic regex/dict, an
  orthogonal concern.
- `nvisy-provider` — that's HTTP / LLM territory; offline NLP is
  separate by design.
- `nvisy-codec` — text is `&str` at this layer; the codec deals
  with raw content.

## Open questions

These are flagged for resolution during implementation, not before:

1. **Concurrent NER calls.** ORT is thread-safe but inference is
   CPU-bound. Should `OrtNerBackend` use a `tokio::Semaphore` to cap
   concurrent inferences? Or let the caller manage? Probably caller's
   problem — backends are passive.
2. **Span alignment between tokens and entities.** When both
   `OrtNerBackend` and `HfTokenizer` use the same tokenizer.json,
   their offsets should agree. Worth documenting but probably not
   worth enforcing.
3. **Memory residency.** ORT sessions and lingua detectors are not
   small. `Arc<dyn NerBackend>` shares one instance across the
   process — sufficient for a single-process server, awkward for
   anything that wants per-tenant isolation. Add when a tenant model
   shows up.

## Out-of-scope clarifications (anti-FAQ)

- *Why not match Presidio 1:1?* Because Presidio's surface assumes
  spaCy. Rust doesn't have spaCy. Matching surface for surface's
  sake produces dummy impls everywhere.
- *Why not wrap PyO3 to spaCy from the start?* Build complexity, GIL
  contention, deployment cost. The trait absorbs a Python backend
  later if someone needs it.
- *Why composable traits when Presidio uses a monolithic
  `NlpEngine`?* Presidio's monolithic engine exists because spaCy is
  monolithic. Rust doesn't have that constraint, and forcing every
  backend to implement NER + tokens + language detection produces
  dummy code. Composable lets each backend do what it does well.
- *Why no model bundling?* Crate size limits (crates.io), download
  costs at install time, air-gapped deployment compatibility, model
  versioning concerns. User-provided paths punt all of this
  correctly.
