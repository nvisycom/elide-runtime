# nvisy-nlp

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

NLP for the Nvisy runtime: named entity recognition, language
detection, and tokenization, composed behind a small set of traits.

This crate hosts the trait surface and the local-by-default
implementations (ONNX-backed NER, lingua language detection, HF and
Unicode tokenizers). LLM-mediated NER lives in `nvisy-llm` — by
deliberate crate split, not by trait restriction: a third-party
backend can implement `NerBackend` over any transport (local model,
HTTP, gRPC) and plug in.

## Traits

- **`NerBackend`** (async) — recognize entities in text. Implemented
  by `OrtNerBackend` (HF token-classification via ONNX), and
  `NoopNerBackend` for tests.
- **`LanguageDetector`** (sync) — detect language + confidence,
  optionally segment mixed-language documents. Implemented by
  `LinguaLanguageDetector`.
- **`Tokenizer`** (sync, fallible) — split text into tokens with
  byte offsets. Implemented by `HfTokenizer` (HF tokenizer.json) and
  `UnicodeTokenizer` (model-free, Unicode word boundaries).

## Quick taste

```rust,ignore
use nvisy_nlp::{NlpEngine, OrtNerBackend, OrtNerConfig, LinguaLanguageDetector};

let ner = OrtNerBackend::new(OrtNerConfig {
    model_path: "models/bert-base-NER.onnx".into(),
    tokenizer_path: "models/tokenizer.json".into(),
    label_map: /* "PER" -> EntityKind::PersonName, ... */,
    max_sequence_length: 512,
    model_name: "dslim/bert-base-NER".into(),
})?;
let language = LinguaLanguageDetector::for_languages(&["en".parse()?, "de".parse()?])
    .expect("at least one supported language");

let engine = NlpEngine::builder()
    .with_ner(ner)
    .with_language_detector(language)
    .build();

let artifacts = engine.analyze("Patient name: John Doe.").await?;
```

## Runtime requirements

`OrtNerBackend` is built against `ort` with `load-dynamic`, which
means **`libonnxruntime` must be installed on the host at runtime**:

- macOS: `brew install onnxruntime`
- Debian/Ubuntu: download from
  [ONNX Runtime releases](https://github.com/microsoft/onnxruntime/releases)

Model files (`.onnx` + `tokenizer.json`) are user-provided. Convert a
HuggingFace token-classification model with
`optimum-cli export onnx --model <name> ./out`.

## Status

Pre-1.0. The trait surface is stable enough to start consuming;
implementations may add fields.
