# Documentation

Reference documentation for the Nvisy inference services.

## Contents

- [`design/`](design) — per-service rationale: the model(s) behind each service
  ([docTR](design/tocr.md), [NER](design/ner.md),
  [PaddleOCR-VL](design/ocrvlm.md)) and what would replace them.

## Wire contract

The contract is defined as pydantic types in
[`nvisy-core`](../packages/nvisy-core) and is task-named (OCR, vision-language
OCR, NER), independent of the engine that implements it:

| Module | Purpose |
| --- | --- |
| [`nvisy_core.ocr.v1`](../packages/nvisy-core/src/nvisy_core/ocr/v1.py) | OCR request/response — `Page → Block → Line → Word` with word-level geometry. |
| [`nvisy_core.ocrvl.v1`](../packages/nvisy-core/src/nvisy_core/ocrvl/v1.py) | Vision-language OCR request/response — block-level regions with text, layout kind, bbox, and reading order. |
| [`nvisy_core.ner.v1`](../packages/nvisy-core/src/nvisy_core/ner/v1) | NER request/response — a schema (entities, classifications, structures) in, model-native results + `modelId` out. |

### NER is schema-driven, labels are model-native

A NER request carries a **schema** — any combination of entity types,
classification tasks, and structured records — and the response returns each
group's results with character offsets and confidence scores. Labels are the
**model's own** (`person`, `email`, `iban`, …), not a normalised taxonomy;
mapping them onto a canonical vocabulary is the **consumer's** job (the Rust
side owns that map, keyed by `modelId`).
