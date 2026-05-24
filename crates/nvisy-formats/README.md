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

Two layers of features:

- **Per-format** (`txt`, `json`, `csv`, `pdf`, …): the granular knob.
  Each pulls just its parent modality's trait surface on
  `nvisy-codec` and that format's specific deps.
- **Per-modality umbrellas** (`text`, `tabular`, `image`, `audio`,
  `rich`): convenience aliases that enable every format in that
  modality. Defaults are `text` + `tabular`.

| Modality | Umbrella | Format features |
|----------|----------|-----------------|
| Text | `text` (default) | `txt`, `json`, `markdown`, `html` |
| Tabular | `tabular` (default) | `csv`, `xlsx` |
| Image | `image` | `png`, `jpeg`, `tiff` |
| Audio | `audio` | `wav`, `mp3` |
| Rich | `rich` | `pdf`, `docx` |

Selecting a single format (e.g. `--features pdf`) compiles only
that loader and pulls only its specific deps (`lopdf`,
`pdfium-render`, `rayon` for PDF). The umbrella feature is just
shorthand for "enable every format in this modality".

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
