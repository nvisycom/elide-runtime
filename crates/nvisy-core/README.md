# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Foundational primitives, the `Modality` marker trait, and the shared
`Error`/`Result` type that every other crate in the Nvisy runtime
builds on.

## Overview

Defines the per-modality marker types (`Text`, `Image`, `Audio`,
`Tabular`) and the `Modality` trait that bundles their associated
location / data / replacement / extraction types. Generic containers
(`Entity<M>`, `Span<M>`, `Annotation<M>`, `Redactions<M>`) live here
and are parameterised on `M: Modality`, which lets every downstream
crate share one shape across modalities.

Also ships the cross-cutting traits the recognizer/extractor
ecosystem reaches for — `EntityRecognizer<M>`, `Extractor<M>`,
`DataAt<M>` / `TextAt<M>`, `RedactAt<M>` — plus the structured
`Error`/`ErrorKind`/`Result` types. The `http` feature exposes a
shared `reqwest_middleware`-based client for crates that talk to
remote services. Depends on no other workspace crate.

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
