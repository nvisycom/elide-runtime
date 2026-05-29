# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Foundational crate for the Nvisy runtime — domain types, error
types, and the cross-cutting traits that every other crate builds
on.

## Overview

`error::*` defines the structured error hierarchy used across the
workspace (pipeline, codec, detection, provider failures). `Error`
carries a sealed kind/message/component/retryable/source tuple,
reachable only through accessors and the
`with_component`/`with_retryable`/`with_source` builders; the
`ErrorKind` variants each have a matching `Error::*` shorthand
(`validation`, `policy`, `not_found`, `connection`, `timeout`,
`cancellation`, `internal`, `runtime`, `serialization`). `Error`
provides a `From<nvisy_ontology::Error>` impl that preserves the
original in the cause chain. The crate ships a `Result<T>` alias
keyed on this error type so every downstream crate can
`use nvisy_core::Result;` and get consistent error semantics.

`content::*` carries the raw-bytes-plus-metadata bundle that every
import flows through: `Content` (the bundle), `ContentData` (the
bytes + SHA-256 + MIME helpers), `ContentMetadata` (filename,
content type, annotations, free-form extras), `ContentSource`
(re-exported from `nvisy-ontology::entity`), and `TextEncoding` (the
encoding flag threaded through text loaders).

`media::*` declares the closed set of recognised document shapes:
`DocumentType` (the top-level kind enum) and the per-category format
enums (`TextFormat`, `ImageFormat`, `AudioFormat`,
`SpreadsheetFormat`, `WordFormat`, `PresentationFormat`) with MIME
and extension dispatch.

`http::*` (behind the `http` feature) ships `build_http_client` — a
free function that returns a `reqwest_middleware::ClientWithMiddleware`
configured with retry and tracing middleware according to a
`HttpConfig`.

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
