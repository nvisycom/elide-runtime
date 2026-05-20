# nvisy-detection

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Entity recognition for the Nvisy runtime: a `Recognizer` trait,
built-in recognizers over `nvisy-nlp`, `nvisy-pattern`, and the LLM
provider, and a `DetectionEngine` that runs them together.

This crate sits between the lower-level NLP / pattern / LLM backends
and the `nvisy-engine` orchestration. Mirrors Presidio's
`presidio-analyzer` package boundary: recognizers are the unit of
plug-in extension; the engine is the unit of orchestration.

## Layers

- **`Recognizer`** trait — `async fn recognize(&self, ctx: &DetectionContext) -> Result<Entities>`.
  Implemented by:
  - `NerRecognizer` — wraps `nvisy_nlp::Engine` (language detection,
    NER, tokens, keywords).
  - `PatternRecognizer` — wraps `nvisy_pattern::PatternEngine`
    (regex, dictionary, allow/deny, context-aware boosting).
  - `LlmRecognizer` — wraps an LLM `NerAgent`.
- **`DetectionContext`** — per-call inputs: text, optional asserted
  language, candidate languages, entity-kind allowlist, score
  threshold, `ScanContext` for pattern hints, correlation UUID.
- **`DetectionEngine`** — holds a `Vec<Arc<dyn Recognizer>>` and
  runs them sequentially against a `DetectionContext`.

`nvisy-engine` wraps a `DetectionEngine` in a single `Operation`
that integrates results into the document envelope.

## Status

Pre-1.0. Trait surface is stable enough to consume; orchestration is
expected to grow (parallel recognizer execution, context-aware
enhancers spanning multiple recognizers).
