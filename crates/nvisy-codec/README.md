# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Modality-keyed handler traits and infrastructure for document
handlers in the Nvisy multimodal redaction platform.

## Overview

This crate defines the trait surface that every concrete document
handler implements:

- `Handler` — base identity + encode.
- `Codable` — codec-side per-modality `Data` + `Redaction` wire
  types.
- `Handle<M>` — single generic per-modality capability trait
  (`locations`, `read`, `redact_at`, `redact`). A multi-modality
  format implements `Handle<M>` once per modality it supports.
- `RichHandle` (behind the `rich` feature) — marker trait
  satisfied by any type that implements
  `Handle<Text> + Handle<Image>`, so PDF/DOCX-style documents fit a
  single trait object.
- `DocumentHandle` — type-erased enum of every modality, with
  uniform `read_*` and `apply_*_redactions` entry points that
  dispatch to the inner `Handle<M>` impl.
- Per-location addressing (`Located<L, D>`, `LocationStream<L>`)
  so detection and redaction can address regions uniformly
  regardless of the source format. `Located<L>` is the bare
  location form; `Located<L, D>` attaches per-location data
  payloads.
- Per-modality redaction wire types (`TextRedaction`,
  `ImageRedaction`, `AudioRedaction`, `TabularRedaction`) and the
  generic `Redactions<S, R>` collection that fuses overlapping
  entries on insert.

Format-specific implementations (PDF, DOCX, HTML, XLSX, PNG, JPEG,
TIFF, WAV, MP3, JSON, Markdown, TXT, CSV) live in the companion
`nvisy-formats` crate. The `impl_image_handler!` macro that
single-image handlers reuse lives there too.

## Feature Flags

Modality features control which trait surface is compiled in. The
default set covers the two lightweight modalities; opt into `image`,
`audio`, or `rich` for the heavier modalities that pull additional
dependencies (`image`, `imageproc`).

| Feature | Default | Description |
|---------|---------|-------------|
| `text` | yes | `Handle<Text>` + `TextData` + `TextRedaction` |
| `tabular` | yes | `Handle<Tabular>` + `TabularRedaction` (pulls `text`) |
| `image` | no | `Handle<Image>` + `ImageData` + `ImageRedaction` (pulls `image` + `imageproc`) |
| `audio` | no | `Handle<Audio>` + `AudioData` + `AudioRedaction` + `sort_redactions_for_audio` |
| `rich` | no | `RichHandle` (pulls `text` + `image`) |

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

