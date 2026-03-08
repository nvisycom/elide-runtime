# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

File-format codecs for the Nvisy multimodal redaction platform.

This crate provides handlers for reading, editing, and writing PDF, DOCX,
HTML, Image, XLSX, Audio, CSV, JSON, and plain-text files. Each handler
implements the `Handler` trait and provides
span-based access to content for detection and redaction.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `pdf` | yes | PDF parsing, text extraction, and page-to-image rendering |
| `docx` | yes | Microsoft Word (.docx) parsing via zip + quick-xml |
| `html` | yes | HTML parsing and text extraction via scraper |
| `xlsx` | yes | Excel (.xlsx) spreadsheet parsing via calamine |

Image, audio, CSV, JSON, and plain-text handlers are always available.

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
