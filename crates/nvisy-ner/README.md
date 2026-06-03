# nvisy-ner

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

NER recognizer and pluggable inference backends for the Nvisy runtime.

## Overview

A trait-driven NER recognizer that turns model-produced spans into typed
entities. One `NerRecognizer` drives any `NerBackend`; shipped backends
are `NoopBackend` (test stub) and the feature-gated `BentoBackend`
(HTTP into the externalised `inference-gliner` service).

Model-bearing backends do **not** run in-process here — inference is
externalised (see [`nvisycom/inference`]). LLM-mediated NER lives in
`nvisy-agent`.

The shared NLP-enrichment side (language detection, tokens, stopwords)
lives in `nvisy_core::nlp` as typed entries on a `TypeMap`; this crate
hosts the producer-side `NlpEngine` trait + a `LinguaNlpEngine`
language-only impl that any downstream consumer reads via the same
type-map.

[`nvisycom/inference`]: https://github.com/nvisycom/inference

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
