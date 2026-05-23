# nvisy-nlp

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

NLP for the Nvisy runtime: named entity recognition, language
detection, and tokenization, composed behind a small set of traits.

## Overview

The crate hosts the trait surface (`NerBackend`, `LanguagePolicy`,
`Tokenizer`) and pluggable implementations: BIO-tagged NER via
`OrtNerBackend` (feature `onnx`), zero-shot NER via `GlinerBackend`
(feature `gliner`), language detection via `LinguaLanguageDetector`,
and tokenization via `HfTokenizer` (feature `onnx`) and
`UnicodeTokenizer`. `NlpEngine` composes one of each into an analysis
pipeline; LLM-mediated NER lives in `nvisy-agent` instead, by
deliberate crate split — a third-party backend can implement
`NerBackend` over any transport (local model, HTTP, gRPC) and plug
in here.

```rust,ignore
use nvisy_nlp::{Context, NlpEngine, LinguaLanguagePolicy, OrtNerBackend, OrtNerConfig};

let ner = OrtNerBackend::new(OrtNerConfig {
    model_path: "models/bert-base-NER.onnx".into(),
    tokenizer_path: "models/tokenizer.json".into(),
    label_map: /* "PER" -> EntityKind::PersonName, ... */,
    max_sequence_length: 512,
    model_name: "dslim/bert-base-NER".into(),
})?;

let engine = NlpEngine::builder()
    .with_ner(ner)
    .with_language_policy(LinguaLanguagePolicy)
    .build()?;

let ctx = Context::builder()
    .with_text("Patient name: John Doe.")
    .with_candidate_languages(vec!["en".parse()?, "de".parse()?])
    .build()?;
let artifacts = engine.analyze(ctx).await?;
```

## Cargo features

- `default = ["test-utils"]` — ships only `NoopNerBackend`, the
  language policy, and the Unicode tokenizer.
- `onnx` — enables `OrtNerBackend` (BIO-tagged BERT-family models)
  and `HfTokenizer`. Pulls in `ort = rc.12`, `tokenizers`, `ndarray`.
- `gliner` — enables `GlinerBackend` (zero-shot NER via `gline-rs`).
  Pulls in `gline-rs` + `orp`, which transitively pin
  `ort = =rc.9`. **Mutually exclusive with `onnx`** — enabling both
  produces a compile-time error.
- `hf` — enables auto-download + SHA-256 verification of manifest-
  defined preset artifacts via `nvisy-core::hf`.
- `test-utils` — exposes `NoopNerBackend` for downstream tests.

## Runtime requirements

Both `OrtNerBackend` and `GlinerBackend` use `ort` with
`load-dynamic`, which means **`libonnxruntime` must be installed on
the host at runtime**:

- macOS: `brew install onnxruntime`
- Debian/Ubuntu: download from [ONNX Runtime releases]

Model files (`.onnx` + `tokenizer.json`) are user-provided. For the
BERT path, convert a HuggingFace token-classification model with
`optimum-cli export onnx --model <name> ./out`. For the GLiNER path,
use a pre-exported model from the [`onnx-community`] org on
HuggingFace (`gliner_small-v2.1`, `gliner_medium-v2.1`, etc).

[ONNX Runtime releases]: https://github.com/microsoft/onnxruntime/releases
[`onnx-community`]: https://huggingface.co/onnx-community

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
