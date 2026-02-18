# nvisy-codec

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

File-format codecs for the Nvisy multimodal redaction platform.

This crate provides handlers for reading, editing, and writing PDF, DOCX,
HTML, Image, XLSX, Audio, CSV, JSON, and plain-text files. Each handler
implements the [`Handler`](crate::handler::Handler) trait and provides
span-based access to content for detection and redaction.
