# Pipeline Fan-Out — Implementation Plan

**Status:** in progress
**Branch:** `feat/pipeline-fanout-scope-c`
**Scope:** Make every pipeline stage genuinely operate on every modality (`Text`, `Tabular`, `Image`, `Audio`), end to end. No stubs, no "modality not yet supported" warnings, no behavior gaps between text and non-text.

This is a working document with a finite lifespan. It exists to keep design decisions on disk while the implementation is in flight, and to make any pickup-after-a-break possible without re-deriving context. Delete after the implementation lands.

---

## 1. Why this work exists

After the import-fan-out PR (#208), the importer correctly produces per-modality envelopes (`AnyEnvelope::{Text, Tabular, Image, Audio}`) from any uploaded file. Rich documents (PDF, DOCX) fan out into a Text + Image envelope pair sharing one underlying `DocumentHandle`. This is the *production*-side fan-out.

But the *consumption* side — the `Orchestrator` and the per-stage pipeline (extraction → detection → deduplication → redaction → validation → export) — is text-only. The orchestrator filters `AnyEnvelope::Text(_)` through `DocumentPipeline::run`; everything else gets dropped with a stub `DocumentResult` whose error reads *"image modality not yet supported by the pipeline."*

That state is dishonest: the engine claims to support multi-modal documents but only the text variant ever produces useful output. Scope C closes the gap.

---

## 2. What "done" looks like

A user uploads a PNG containing the text *"Patient: Jane Doe, DOB 1985-03-14, SSN 123-45-6789"*. The current behavior is: PNG decoded → `DocumentEnvelope<Image>` produced by importer → orchestrator emits "modality not yet supported" → nothing happens. After Scope C: PNG → `DocumentEnvelope<Image>` → OCR populates `envelope.document` with `ImageBlock::Text { region, text }` blocks → detection scans those text fields and produces `Entity<Image>` whose `location` is the *region* (not a byte offset) → redaction calls `Handle<Image>::redact_at(region, ImageRedaction::Block { color: ... })` which mutates the actual PNG pixels → validation re-OCRs the redacted PNG and confirms the SSN no longer appears → export writes the redacted PNG to the registry.

Same shape for audio (STT → AudioBlock::Speech → text-scan → AudioRedaction::Silence → re-STT validation), tabular (already structured, no extraction needed), and the rich-document case where one upload produces both a Text envelope (PDF text layer) and an Image envelope (rendered pages → OCR).

---

## 3. Architectural decisions

These are the choices made up front, in writing, so I don't drift mid-implementation. Each is the one we agreed on; the alternatives are noted for the future reader who wonders why.

### 3.1 Detection-over-embedded-text: text-typed recognizers + per-modality detection drivers (D1.A)

**Decision:** Pattern and NER recognizers stay typed over `Text`. A new `DetectionDriver<M>` per modality walks `envelope.document.blocks`, extracts text from each block via a per-modality accessor (`ImageBlock::text()`, `AudioBlock::text()`, etc.), runs the text recognizers on that text, and **converts each resulting `Entity<Text>` into `Entity<M>` by attaching the source block's location** (or, where `Block<M>::spans` exist, computing a finer-grained location for the matched byte range via the span map).

**Why this and not "recognizers become generic over M":** The recognizer surface is settled, well-tested, and consumed by external configs. Making recognizers generic in M would force every backend (pattern, GLiNER, Bento NER) to grow per-modality impls that all reduce to "extract text, scan it." That's the wrong place to put the per-modality logic — it belongs at the pipeline layer where the block walk happens. The per-modality driver is the single place that knows "how do I get scannable text from a `Block<M>`."

**Consequence:** When `Block<M>::spans` is empty, the entity's location is the *whole block's* region/time-span. That's coarser than text-byte-offset detection. Acceptable for v1; finer-grained location-mapping requires recognizers to emit spans, which is a separate product change.

### 3.2 Redaction applicators: real byte mutation via Handle<M>::redact_at (D2.A — finish #196)

**Decision:** The redaction stage actually calls `envelope.handle.lock().await.redact_at(loc, redaction)` for every modality. This subsumes task #196: the previously-stubbed *applicator* gets wired through to the codec, which already has the per-format mutating implementations (text byte-range replace, image bounding-box blur/block/pixelate via `image::redact::apply`, audio sample-range silence/remove via `audio::redact::apply`, tabular cell-level replace).

**Why this and not "stub non-text redaction":** Scope C is explicitly "no corners, no stubs." The codec-layer machinery is already real for every modality; not wiring it up would leave the engine's pretty audit output disconnected from actual byte changes. Half-done redaction is worse than no redaction because it lies in the user-facing audit.

### 3.3 Validation: per-modality re-scan, opt-in via config (D3.B with cost guard)

**Decision:** Validation re-encodes the redacted envelope, re-extracts (re-OCR for image, re-STT for audio, no-op for text/tabular), re-detects, and asserts that no originally-detected values still appear. Failures produce `AuditEntry::ValidationFailure { entity_id, reason }` records.

**Cost guard:** Re-OCR on a large image / re-STT on long audio is *seconds* of compute, vs. *microseconds* for text validation. The existing `Validation` config grows a per-modality knob — `re_scan: bool` — defaulting to `true` for text/tabular (cheap) and `false` for image/audio (expensive). Users who want strict guarantees for image/audio opt in explicitly.

**Why opt-in and not always-on:** Real production deployments will not pay seconds-per-document for validation unless they specifically need post-redaction proof. Defaulting it on would make image/audio pipelines mysteriously slow.

### 3.4 Export: codec already handles all modalities

**Decision:** Export writes `envelope.encode()` bytes to the registry. The codec's `Handler::encode` is already generic per format; no engine-layer change is needed beyond confirming the existing exporter doesn't have hidden text-specific assumptions.

**Audit before declaring done:** Read `crates/nvisy-engine/src/ingestion/exporter.rs` end-to-end to confirm. If it does have text-specific logic, document it here and adjust scope.

### 3.5 Orchestrator dispatch: trait-based (A.1 from earlier discussion)

**Decision:** Define a `Pipeline` trait whose `run(envelope) -> DocumentResult` method is implemented by `DocumentPipeline<M>` for every M. The orchestrator matches `AnyEnvelope` once to extract the concrete envelope, then spawns the matching `DocumentPipeline<M>::run` task. Each pipeline task is a fully-typed, M-monomorphized execution.

**Why this and not "match in the orchestrator with per-modality run methods":** The matching-in-orchestrator approach is simpler today but creates duplicated stage-dispatch logic per modality. The trait-based approach has a single generic `DocumentPipeline<M>` whose body is shared across all modalities, with stage-level genericity carrying the per-modality differences. Aligns with the "no shortcuts" mandate.

### 3.6 Extractor dispatch: per-modality registry, selected by envelope's M

**Decision:** `Extractors::run(envelope)` becomes generic in M and dispatches via an internal trait:

- `Extractors::run::<Text>(envelope)` → no-op (text already structured).
- `Extractors::run::<Tabular>(envelope)` → no-op (tabular already structured).
- `Extractors::run::<Image>(envelope)` → invoke OCR extractor (`OcrExtractor::extract`), build `Document<Image>` with `ImageBlock`s, store on `envelope.document`.
- `Extractors::run::<Audio>(envelope)` → invoke STT extractor (`SttExtractor::extract`), build `Document<Audio>` with `AudioBlock`s, store on `envelope.document`.

When the configured extractor for a modality is missing (e.g. user didn't configure `OcrExtractor`), the stage logs a warning and proceeds with `envelope.document = None`. Downstream detection on an empty document produces zero entities — no error, no panic, but a clear audit-log signal that extraction was skipped.

---

## 4. Per-stage rewrite plan

Each stage gets its own subsection. The order below is the order in which the implementation should land within the PR (dependency order), but reviewers should read them all before approving the PR.

### 4.1 `DocumentPipeline<M>` generic

**Current state:** `crates/nvisy-engine/src/pipeline/orchestrator.rs::DocumentPipeline` is hardcoded `<Text>`. Six stage methods (`run_extraction`, `run_detection`, `run_dedup`, `run_redaction`, `run_validation`, `run_exports`) are all `<Text>`-typed.

**Change:** `pub struct DocumentPipeline<M: Modality + …>`. Each stage method becomes generic-friendly (bound on whatever traits the stage requires for M). The body of `DocumentPipeline::run` is a sequence of stage calls that compose; the only per-modality logic should be *inside* each stage, not in the pipeline driver.

**Trait bounds we'll need on M** (to confirm during implementation): `Modality`, `Default` (for `inclusion_entities` sentinel locations), whatever the per-stage execute methods need.

### 4.2 `Orchestrator::run` dispatches by variant to typed pipelines

**Current state:** matches `AnyEnvelope::Text` and ignores the rest with a warning.

**Change:**
```rust
match envelope {
    AnyEnvelope::Text(env)    => spawn_pipeline::<Text>(env, ...),
    AnyEnvelope::Tabular(env) => spawn_pipeline::<Tabular>(env, ...),
    AnyEnvelope::Image(env)   => spawn_pipeline::<Image>(env, ...),
    AnyEnvelope::Audio(env)   => spawn_pipeline::<Audio>(env, ...),
}
```
where `spawn_pipeline::<M>` constructs the typed `DocumentPipeline<M>`, runs it, and produces a `DocumentResult` whose envelope field is some modality-erased shape (probably `Option<AnyEnvelope>` rather than `Option<DocumentEnvelope<Text>>`).

**Consequence:** `DocumentResult::envelope` changes type from `Option<DocumentEnvelope<Text>>` to `Option<AnyEnvelope>`. Downstream consumers of `RunOutput` (the server's response handlers, the CLI's printers) need to handle all variants. Audit those call sites.

### 4.3 Extraction stage

**Current state:** `Extractors::run` takes `<Text>` envelope and is a no-op.

**Change:** Make `Extractors::run` generic in M, with an internal match that dispatches to the right per-modality extractor (or no-op for Text/Tabular). Wire `OcrExtractor::extract(envelope.handle, ...)` for Image and `SttExtractor::extract(envelope.handle, ...)` for Audio.

**Open question for implementation:** OCR and STT extractors need to read bytes from the codec handle. The current `Handle<Image>::read(&loc) -> ImageData` returns one region at a time; for OCR we want the *whole image bytes* (or all blocks). Confirm whether `Handle<Image>::encode()` produces what OCR expects, or whether we need a different accessor. Same question for `Handle<Audio>`.

### 4.4 Detection stage

**Current state:** `DetectionEngine::detect(envelope: &mut DocumentEnvelope<Text>)`, calls text recognizers directly on the codec handle.

**Change:** Generic `DetectionDriver<M>` (or similar name) per the decision in §3.1. Walks `envelope.document.blocks`. For each block:
1. Extract scannable text via a per-M accessor (a small trait `BlockText: fn text(&self) -> Option<&str>` impl'd for each `M::Block`).
2. Run the configured recognizers on that text.
3. For each `Entity<Text>` returned, build an `Entity<M>` whose location is computed by `entity_location_from_text_match<M>(block, text_match) -> M` (uses block.spans when present, falls back to block-region/whole-span).
4. Push into `block.entities`.

**New small trait surface in nvisy-ontology:** `BlockText` accessor on each `M::Block`. Image variants `Text`/`Heading`/`Table` return their text field; non-text variants (`Figure`/`Separator`/etc.) return None. Audio: `Speech` returns text; `Silence` returns None. Tabular blocks similarly. This is the only ontology-side change in the whole arc.

**Recognizers themselves do not change.** The pattern engine and NER backends keep their text-typed API.

### 4.5 Deduplication stage

**Current state:** Already generic in M, wired through `ValueAt<M>`. No change needed.

**Confirm:** Run the existing dedup tests with non-text envelopes to verify the generic surface actually compiles when M = Image/Audio. If `ValueAt<Image>` is fully implemented (it is, per the earlier ValueAt collapse), this is free.

### 4.6 Redaction stage

**Current state:** `Redactor::execute(envelope: &mut DocumentEnvelope<Text>)`. Evaluates redaction policies (already generic — uses `Policy<M>` via `PolicyStore`) but the *applicator* (the code that actually calls `Handle::redact_at`) is stubbed (per task #196).

**Change:** `Redactor<M>` generic. The evaluator stays. The applicator is finished — for each redacted entity, look up the redaction, call `envelope.handle.lock().await.redact_at(entity.location, redaction)`. The codec's `Handle<M>::redact_at` is already implemented per-format in nvisy-formats; this stage is just wiring.

**Subsumes task #196.**

### 4.7 Validation stage

**Current state:** `Validator::execute(envelope: &mut DocumentEnvelope<Text>)`. Re-scans the encoded text to verify no detected values remain.

**Change:** Generic `Validator<M>`. New `Validation::re_scan_per_modality` config (HashMap-ish: per-modality bool, defaulting to `{text: true, tabular: true, image: false, audio: false}` per §3.3). When enabled:
1. Re-encode the envelope via `envelope.handle.encode()`.
2. Re-extract (re-OCR for image, re-STT for audio, no-op for text/tabular).
3. Re-run detection.
4. For each originally-detected entity (looked up from `audit.entities` before redaction; we'll need to snapshot them), assert no overlapping match appears in the re-detection result.
5. Failures become `AuditEntry::ValidationFailure` records.

**Open implementation question:** the existing validator uses the recognizers directly. To re-detect we'd reuse the same `DetectionDriver<M>` from §4.4. Confirm the validator can hold a reference to the detection engine, or whether we plumb it through the constructor.

### 4.8 Export stage

**Current state:** `Orchestrator::run_exports(plan.exports, envelope: &DocumentEnvelope<Text>)`. Writes the encoded bytes to the registry.

**Change:** Generic in M. Per §3.4 the codec already handles all modalities at the encode level; we just need the export path's type signature to be generic, not Text-specific.

**Audit step:** Read `crates/nvisy-engine/src/ingestion/exporter.rs` and confirm. If anything text-specific surfaces, document it here.

---

## 5. Test strategy

Each stage gets unit tests for its generic surface; the pipeline gets one end-to-end integration test per modality.

### 5.1 Per-stage unit tests

- **Detection driver**: synthesize a `Block<Image>::Text { region, text: "John Smith works at Acme" }`, run detection, assert `Entity<Image>` produced with the expected region and entity kind.
- **Redaction applicator**: build a `DocumentEnvelope<Image>` with a known entity, run redaction, assert the codec handle was actually mutated (compare bytes pre/post).
- **Validation re-scan**: build an envelope that would fail validation (e.g. redaction with a value that's still present), run validation, assert `AuditEntry::ValidationFailure` produced.

### 5.2 End-to-end integration tests

One per modality, all in `crates/nvisy-engine/tests/pipeline_<modality>.rs`:

- `pipeline_text.rs`: existing path, regression-proof — should keep passing unchanged.
- `pipeline_tabular.rs`: CSV with PII in cells → detection finds it → redaction blanks the cells → encoded CSV no longer contains the values.
- `pipeline_image.rs`: PNG with embedded text → OCR populates blocks → detection finds PII → redaction blocks the region → encoded PNG has the region painted over. Uses the noop OCR backend for unit-level correctness; a feature-gated `bento` variant for real OCR if the user runs the test with the Bento service.
- `pipeline_audio.rs`: small WAV with synthesized speech → STT populates blocks → detection finds PII in transcript → redaction silences the time-span → encoded WAV has the segment silenced. Noop STT default; feature-gated real STT.

### 5.3 Test infrastructure additions

- **Synthetic fixtures**: a 32×32 PNG with rasterized text (generated at test time via the `image` crate), a 1-second WAV with a known sine wave (generated via `hound`). These live under `crates/nvisy-engine/testdata/`.
- **Noop OCR/STT outputs**: the noop extractors today return empty results. For the integration test we need them to return *deterministic mock output* matching the fixture text. Either: a fixed-output noop variant (`NoopOcrBackend::with_mock_text(s)`) used only in tests, or a small `MockOcrExtractor` that lives in the engine's test_utils. Decision deferred to implementation time; flag here for visibility.

---

## 6. Out of scope (explicit non-goals for this arc)

Listed so reviewers don't ask "why didn't you also do X":

- **CSV header support for the dictionary loader** (issue #207). Orthogonal; doesn't gate this work.
- **Drop-reason telemetry for recognizers** (task #112). The detection driver's introduction is a natural future home for this, but emitting per-drop traces is a separate scope.
- **Recognizers becoming generic over M.** Per §3.1; explicit alternative we rejected.
- **Re-routing rich-document Image envelopes around OCR when the rich doc has a usable text layer.** The current import-fan-out always produces both envelopes; the Image envelope will go through OCR even though the PDF's text layer was already extracted into the Text envelope. Acceptable double-work for v1; file follow-up issue.
- **Per-block confidence aggregation across modalities.** The block-level entity attachment may produce duplicate entities (same value, different blocks). Dedup handles within-modality dedup; cross-modality dedup is out of scope.

---

## 7. Risks and known unknowns

Things I might discover mid-implementation that could expand scope:

- **The exporter has hidden text assumptions.** If `Exporter::export` does anything beyond `envelope.handle.encode()`, I'll need to factor it. (Mitigation: read it first; document findings here before writing code.)
- **The validation re-scan path needs the detection engine, which needs the extractors, which means the validator's constructor signature changes.** This may ripple into the orchestrator and runtime config plumbing. (Mitigation: design the validator as taking an `Extractors` + recognizers reference at construction time, not at execute time.)
- **The trait-based pipeline (§3.5) may require a `Box<dyn Pipeline>` or a trait method that's hard to make object-safe given `&mut envelope` flow.** If trait-object dispatch breaks down, fall back to a `match` in `Orchestrator::run` that statically dispatches to monomorphized `DocumentPipeline<M>::run` calls — same end result, less abstraction.
- **The `DetectionDriver<M>` needs a way to map text matches back to source locations using `Block<M>::spans`.** The `Span<M>` shape exists but I haven't yet confirmed it carries enough info to do byte-range-to-region mapping. (Mitigation: read `nvisy-ontology::document::Span` end-to-end before §4.4.)
- **Image OCR test fixtures: rendering text into a PNG at test time means depending on a font.** If `image` doesn't bundle one, may need a small bundled `.ttf` (license question) or skip the integration test in favor of a hand-crafted fixture image. (Mitigation: try the simplest path first — `image::ImageBuffer::from_pixel` for a colored rectangle won't have detectable text, but a checked-in PNG with text would. Probably check in a small fixture.)

---

## 8. Sequencing within the PR

Implementation order (each step ends with everything still compiling and tests passing):

1. **§4.1** — make `DocumentPipeline<M>` generic. Existing text path still compiles; non-text variants still bail out at the orchestrator. Smallest non-trivial change.
2. **§4.2** — orchestrator dispatches by variant. `DocumentResult::envelope` changes type. Downstream consumers updated. Non-text envelopes now flow into typed pipelines that mostly do nothing yet.
3. **§4.5** — confirm dedup works for all M (probably needs zero code changes; this is a "run the tests and look at the failures" step).
4. **§4.3** — wire extraction for image/audio. After this, image/audio envelopes have populated `Document<M>` after the extraction stage.
5. **§4.4** — detection driver. After this, entities are produced for all modalities.
6. **§4.6** — redaction applicator. After this, bytes actually change for all modalities. Subsumes #196.
7. **§4.7** — validation re-scan. After this, full audit fidelity for opted-in modalities.
8. **§4.8** — export generification (likely small).
9. **§5** — end-to-end integration tests + per-stage unit tests.

Commit-by-commit on the branch maps roughly to this sequence. The PR description will summarize the whole arc; individual commit messages cover each step.

---

## 9. Done definition

The PR is mergeable when:

- All four `pipeline_<modality>.rs` integration tests pass against the noop extractors.
- `cargo test --workspace --all-features` is green.
- `cargo clippy --workspace --all-features --all-targets -- -D warnings` is clean.
- `cargo doc --workspace --all-features --no-deps` has zero warnings.
- Task #197 (extraction fan-out) and #196 (redaction applicator) can both be marked completed.
- `Orchestrator::run` no longer emits any "modality not yet supported" stub errors.
- This planning doc is deleted in the same PR — it has served its purpose.
