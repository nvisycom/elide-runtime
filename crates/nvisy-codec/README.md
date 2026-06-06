# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Format handlers, the `Handle<M>` / `IndexedHandle<M>` trait pair, and
the `CodecRegistry` that drives ingest in the Nvisy runtime.

## Overview

Built-in handlers cover TXT, JSON, Markdown, HTML, CSV, XLSX, PNG,
JPEG, TIFF, WAV, MP3, PDF, and DOCX. Each handler implements a
streaming `Handle<M>` (chunked decode), an optional random-access
`IndexedHandle<M>` (read regions, apply redactions), and is
registered into `CodecRegistry` under a `FormatId` with extension +
content-type hints. Consumers decode through the registry and get
back an `UntypedDocumentHandle` they can downcast to a typed
`DocumentHandle<M>` or wrap in a `DecodedBuffer<M>`.

Also owns the `content` module — `Content`, `ContentData`,
`ContentMetadata`, `ContentSource`, `TextEncoding` — the raw-bytes
side of the import surface. Each file format is feature-gated
(`txt`, `csv`, `png`, …) with umbrella features `text`, `tabular`,
`image`, `audio`, `rich` for groups. Depends only on `nvisy-core`.

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
