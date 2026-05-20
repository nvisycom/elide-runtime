# nvisy-nlp

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Offline NLP for the Nvisy runtime: named entity recognition, language
detection, and tokenization, composed behind a small set of traits.

See [`DESIGN.md`](DESIGN.md) for the full architecture, the trait
surface, and the dependency audit.

## Quick taste

```rust,ignore
use nvisy_nlp::{NlpEngine, OrtNerBackend, OrtNerConfig, LinguaLanguageDetector};

let ner = OrtNerBackend::new(OrtNerConfig {
    model_path: "models/bert-base-NER.onnx".into(),
    tokenizer_path: "models/tokenizer.json".into(),
    label_map: /* "PER" -> EntityKind::PersonName, ... */,
    max_sequence_length: 512,
})?;
let language = LinguaLanguageDetector::for_languages(&["en".parse()?, "de".parse()?]);

let engine = NlpEngine::builder()
    .with_ner(ner)
    .with_language_detector(language)
    .build();

let artifacts = engine.analyze("Patient name: John Doe.").await?;
```

## Status

Pre-1.0. The trait surface is stable enough to start consuming;
implementations may add fields. See the design doc's "open questions"
section for what's still in flux.

## Layout

```text
src/
├── lib.rs
├── error.rs        NlpError
├── artifacts.rs    NlpArtifacts, Token
├── engine.rs       NlpEngine composite
├── ner/            NerBackend trait + OrtNerBackend, NoopNerBackend
├── language/       LanguageDetector trait + LinguaLanguageDetector
└── tokenizer/      Tokenizer trait + UnicodeTokenizer, HfTokenizer
```
