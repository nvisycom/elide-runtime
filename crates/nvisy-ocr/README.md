# nvisy-ocr

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

OCR provider integrations for the Nvisy runtime. Wraps third-party
OCR services behind a uniform `OcrEngine` / `Backend` surface.

This crate replaces the OCR half of the former `nvisy-provider`.
LLM-mediated entity verification (the LLM-side counterpart that
verifies OCR-proposed entities) lives in `nvisy-rig`.

## Providers

Built-in (default):
- **Surya** (`SuryaBackend`) — Datalab Surya HTTP backend.
- **PaddleX** (`PaddleXBackend`) — Baidu PaddleX HTTP backend.

Behind feature flags:
- **AWS Textract** (`aws-textract`) — `AwsTextractBackend`.
- **Google Cloud Vision** (`google-vision`) — `GoogleVisionBackend`.
- **Azure Document Intelligence** (`azure-docai`) — `AzureDocaiBackend`.

## Public surface

The crate re-exports its `ocr::*` module at the crate root, so
`use nvisy_ocr::{OcrEngine, OcrProvider, Backend, ImageInput, ...}`
works directly.

## HTTP

Owns an internal `HttpClient` over `reqwest-middleware` with retry +
tracing. Each backend constructs its own client from config; nothing
is shared across providers or with `nvisy-rig`.
