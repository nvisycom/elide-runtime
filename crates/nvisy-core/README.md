# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Foundational crate for the Nvisy runtime — domain types, error
types, and the cross-cutting traits that every other crate builds
on.

## Overview

`Error` and `ErrorKind` define the structured error hierarchy used
across the workspace (pipeline, codec, detection, provider
failures), with `with_component`/`with_retryable` helpers and
unconditional `From` conversions from `nvisy_ontology::Error`. The
crate ships a `Result<T>` alias keyed on this error type so every
downstream crate can `use nvisy_core::Result;` and get consistent
error semantics.

`content::*` defines the span-based representation of document
content shared across modalities (text, image, audio, structured),
`media::*` carries the unified `File` type with MIME detection +
byte-level access, and `detection::*` hosts the `Recognizer` trait
(with associated `type Context`) plus `DetectionParams` — the
abstraction every detection backend implements regardless of which
crate it lives in.

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
