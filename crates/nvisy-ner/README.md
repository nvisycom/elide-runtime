# nvisy-ner

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

NER traits and language infrastructure for the Nvisy runtime.

## Overview

This crate hosts:

- `Backend` trait — the abstraction every NER source plugs into.
- `NoopBackend` — empty implementation used by tests and by
  deployments that detect via patterns / LLM only.
- `BentoBackend` (feature `bento`) — calls the externalized
  `inference-gliner` Bento in [`nvisycom/inference`] over HTTP.
- `LanguagePolicy` trait + `LinguaLanguagePolicy` — language detection
  via Lingua.
- `NerEngine` — composes a NER backend and a language policy into a
  single `analyze(text, ctx)` call.

Model-bearing NER backends do **not** live in-process here. Inference
is externalized to a separate service (see [`nvisycom/inference`]) and
called over HTTP via `BentoBackend`. In-process backends (BERT-NER
over `ort`, GLiNER via `gline-rs`) lived here previously and have
been removed in favour of the externalized service.

LLM-mediated NER lives in `nvisy-agent`.

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
