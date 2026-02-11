# nvisy-ingest

File-format loaders for the Nvisy multimodal redaction platform.

This crate provides loaders for PDF, DOCX, HTML, Image, Parquet, XLSX,
Audio, CSV, JSON, and plain-text files. Each loader implements the
[`Loader`](crate::loaders::Loader) trait and converts raw
blob bytes into structured `Document`, `ImageData`, or `TabularData`
artifacts.
