# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Modality-keyed handler traits and infrastructure for document
handlers in the Nvisy multimodal redaction platform.

## Overview

This crate defines the trait surface that every concrete document
handler implements:

- [`Handler`] — base identity + encode.
- [`Codable`] — codec-side per-modality `Data` + `Redaction` wire
  types.
- [`Handle<M>`] — single generic per-modality capability trait
  (`locations`, `read`, `redact_at`, `redact`). A multi-modality
  format implements `Handle<M>` once per modality it supports.
- [`RichHandle`] — marker trait satisfied by any type that
  implements `Handle<Text> + Handle<Image>`, so PDF/DOCX-style
  documents fit a single trait object.
- [`DocumentHandle`] — type-erased enum of every modality, with
  uniform `read_*` and `apply_*_redactions` methods.
- Span-based addressing (`Span`, `Located`, `LocationStream`) so
  detection and redaction can address regions uniformly regardless
  of the source format.
- Per-modality redaction shapes (`TextRedaction`, `ImageRedaction`,
  …) and `apply_*_redaction` helpers that concrete handlers reuse.

Format-specific implementations (PDF, DOCX, HTML, XLSX, PNG, JPEG,
TIFF, WAV, MP3, JSON, Markdown, TXT, CSV) live in the companion
[`nvisy-formats`] crate.

## Feature Flags

Modality features control which trait and helper set is compiled in.
The default set covers the two lightweight modalities; opt into
`image`, `audio`, or `rich` for the heavier modalities that pull
additional dependencies (`image`, `imageproc`).

| Feature | Default | Description |
|---------|---------|-------------|
| `text` | yes | `Handle<Text>` impls + `TextRedaction` + `apply_text_redaction` |
| `tabular` | yes | `Handle<Tabular>` impls + `TabularRedaction` + `apply_tabular_redaction` (pulls `text`) |
| `image` | no | `Handle<Image>` impls + `ImageData` + `apply_image_redaction` + `impl_image_handler!` (pulls `image` + `imageproc`) |
| `audio` | no | `Handle<Audio>` impls + `AudioData` + `apply_audio_redaction` |
| `rich` | no | [`RichHandle`] (pulls `text` + `image`) |

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

[`Handler`]: handler::Handler
[`Codable`]: core::Codable
[`Handle<M>`]: core::Handle
[`RichHandle`]: handler::RichHandle
[`DocumentHandle`]: DocumentHandle
[`nvisy-formats`]: https://docs.rs/nvisy-formats
