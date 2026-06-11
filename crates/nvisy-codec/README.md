# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Format handlers, the `Handler<M>` trait, and the `CodecRegistry`
that drives ingest in the Nvisy runtime.

## Overview

Built-in handlers cover TXT, JSON, Markdown, HTML, CSV, XLSX, PNG,
JPEG, TIFF, WAV, MP3, PDF, and DOCX. Each implements `Handler<M>`
(streaming `next_chunk`, random-access `read` / `redact`,
`lift_chunk` for offset translation) and pairs with a `Loader<M>`
that decodes raw bytes into the handler. A `Format` descriptor
built via `Format::new::<M, _>(id, loader)` plus chained
`.with_extensions(...)` / `.with_content_types(...)` registers the
pair into `CodecRegistry`.

Consumers resolve a `Format` by extension, content-type, or id and
get back an `UntypedDocumentHandle` they commit to a modality via
`into_text` / `into_tabular` / `into_image` / `into_audio`. The
typed `DocumentHandle<M>` implements `nvisy-core`'s `TextAt` /
`DataAt` / `RedactAt` directly, so pipeline components read from
and write to codec-backed sources through the same traits the
engine bounds on. The `content` module (`Content`, `ContentData`,
`ContentDescriptor`, `ContentDigest`, `ContentRecord`,
`ContentSource`, `TextEncoding`) carries the raw-bytes side of the
import surface. Each format is feature-gated (`txt`, `csv`,
`png`, …) with umbrella features `text`, `tabular`, `image`,
`audio`, `rich`. Depends only on `nvisy-core`.

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
