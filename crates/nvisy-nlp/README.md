# nvisy-nlp

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

NLP for the Nvisy runtime: named entity recognition, language
detection, and tokenization, composed behind a small set of traits.

## Overview

The crate hosts the trait surface (`NerBackend`, `LanguageDetector`,
`Tokenizer`) and the local-by-default implementations (ONNX-backed
NER via `OrtNerBackend`, language detection via
`LinguaLanguageDetector`, tokenization via `HfTokenizer` and
`UnicodeTokenizer`). `Engine` composes one of each into an analysis
pipeline; LLM-mediated NER lives in `nvisy-agent` instead, by
deliberate crate split — a third-party backend can implement
`NerBackend` over any transport (local model, HTTP, gRPC) and plug
in here.

```rust,ignore
use nvisy_nlp::{Context, Engine, LinguaLanguagePolicy, OrtNerBackend, OrtNerConfig};

let ner = OrtNerBackend::new(OrtNerConfig {
    model_path: "models/bert-base-NER.onnx".into(),
    tokenizer_path: "models/tokenizer.json".into(),
    label_map: /* "PER" -> EntityKind::PersonName, ... */,
    max_sequence_length: 512,
    model_name: "dslim/bert-base-NER".into(),
})?;

let engine = Engine::builder()
    .with_ner(ner)
    .with_language_policy(LinguaLanguagePolicy)
    .build()?;

let ctx = Context::builder()
    .with_text("Patient name: John Doe.")
    .with_candidate_languages(vec!["en".parse()?, "de".parse()?])
    .build()?;
let artifacts = engine.analyze(ctx).await?;
```

## Runtime requirements

`OrtNerBackend` is built against `ort` with `load-dynamic`, which
means **`libonnxruntime` must be installed on the host at runtime**:

- macOS: `brew install onnxruntime`
- Debian/Ubuntu: download from [ONNX Runtime releases]

Model files (`.onnx` + `tokenizer.json`) are user-provided. Convert
a HuggingFace token-classification model with
`optimum-cli export onnx --model <name> ./out`.

[ONNX Runtime releases]: https://github.com/microsoft/onnxruntime/releases

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
