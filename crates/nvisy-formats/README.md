# nvisy-formats

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Format-specific loaders and handlers for the Nvisy multimodal
redaction platform.

## Overview

This crate provides concrete implementations of the trait surface
defined in [`nvisy-codec`] for every supported file format:

| Modality | Formats |
|----------|---------|
| Text | TXT, JSON, Markdown, HTML |
| Tabular | CSV, XLSX |
| Image | PNG, JPEG, TIFF |
| Audio | WAV, MP3 |
| Rich | PDF, DOCX |

The entry point is [`decode`], which dispatches a [`Content`] to the
appropriate loader by its detected document type and returns a
ready-to-use [`ContentHandle`].

## Feature Flags

Each format is its own feature. Modality features (`text`,
`tabular`, `image`, `audio`, `rich`) are usually pulled in
transitively by the per-format features but can also be enabled
directly to compile just the modality infrastructure in
`nvisy-codec`.

The default set covers the four lightweight formats whose modalities
are also `nvisy-codec`'s defaults (`text` + `tabular`):

| Feature | Default | Pulls |
|---------|---------|-------|
| `txt` | yes | `nvisy-codec/text` |
| `json` | yes | `nvisy-codec/text` |
| `markdown` | yes | `nvisy-codec/text` |
| `csv` | yes | `nvisy-codec/tabular`, `csv` |
| `html` | no | `nvisy-codec/text`, `scraper` |
| `xlsx` | no | `nvisy-codec/tabular`, `calamine` |
| `png` | no | `nvisy-codec/image`, `image/png` |
| `jpeg` | no | `nvisy-codec/image`, `image/jpeg` |
| `tiff` | no | `nvisy-codec/image`, `image/tiff` |
| `wav` | no | `nvisy-codec/audio`, `hound` |
| `mp3` | no | `nvisy-codec/audio`, `hound` |
| `pdf` | no | `nvisy-codec/rich`, `lopdf`, `pdfium-render`, `rayon` |
| `docx` | no | `nvisy-codec/rich`, `zip`, `quick-xml` |

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

[`nvisy-codec`]: https://docs.rs/nvisy-codec
[`Content`]: https://docs.rs/nvisy-core/latest/nvisy_core/content/struct.Content.html
[`ContentHandle`]: https://docs.rs/nvisy-codec/latest/nvisy_codec/enum.ContentHandle.html
