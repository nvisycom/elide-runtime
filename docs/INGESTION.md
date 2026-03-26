# Ingestion & Transformation

## 1. Overview

The ingestion layer is responsible for accepting content from heterogeneous sources and normalizing it into a unified internal representation suitable for downstream detection and redaction. The transformation layer handles the inverse concern: producing redacted output in the appropriate format while preserving the structural integrity of the original document.

The quality of the ingestion layer is a critical success factor. Redaction platforms that cannot reliably parse and extract content from real-world documents — scanned forms, embedded tables, multi-speaker audio — will produce incomplete redaction results regardless of the sophistication of their detection models.

## 2. Supported Input Formats

The platform must support ingestion across multiple modalities. Formats are organized into tiers reflecting implementation priority and expected coverage at each stage of the product lifecycle.

### Tier 1: Core (launch requirement)

These formats represent the most common inputs in regulated enterprise environments and must be supported at general availability:

- **PDF**: Native (digitally authored) and scanned, including multi-page documents with mixed content (text, images, tables, forms).
- **Images**: JPG, PNG, and TIFF: the dominant formats for scanned documents, photographs, and screenshots.
- **Plain text and markup**: TXT, HTML, and Markdown.
- **Structured data**: CSV and JSON.
- **Office documents**: DOCX, XLSX, PPTX.
- **Audio**: WAV, MP3, and other common audio formats.

### Tier 2: Extended (near-term)

These formats are frequently encountered in enterprise workflows and should be supported shortly after launch:

- **Email**: EML and MSG formats, including inline content and attachments (recursively ingested).
- **Communications**: Chat log exports from Slack, Teams, and WhatsApp.

### Tier 3: Specialized (roadmap)

These formats address long-tail use cases in specific verticals or operational contexts:

- **Archival and compound formats**: ZIP, TAR, and other container formats with recursive extraction of enclosed files.
- **Domain-specific**: DICOM (medical imaging), GeoTIFF (geospatial), and other vertical-specific formats as demand dictates.

## 3. Extraction Capabilities

Each modality requires specialized extraction techniques:

- **Optical character recognition (OCR)**: Layout-aware OCR that preserves spatial relationships between text regions, table cells, headers, and form fields.
- **Speech-to-text**: Transcription with speaker diarization, enabling attribution of spoken content to individual speakers.
- **Entity identification in images**: Detection and localization of entities within images: faces, persons, objects, text regions, documents, and other identifiable elements, producing bounding boxes or segmentation masks that downstream detection and redaction stages can operate on.
- **Document structure parsing**: Identification of semantic document elements: headings, paragraphs, tables, lists, and form fields, beyond raw text extraction.
- **Metadata extraction**: Capture of authorship, timestamps, geolocation, and other embedded metadata that may itself constitute sensitive information.

## 4. Transformation & Output

Following redaction, the transformation layer must produce output that meets downstream requirements while maintaining fidelity to the original format.

### 4.1 Format Preservation

Redacted output should preserve the structural characteristics of the source document. Tables must remain aligned, page layouts must be maintained, and non-redacted content must remain unaltered.

### 4.2 Output Formats

The primary output of the transformation layer is a redacted file in the same format as the input — a PDF produces a redacted PDF, an image produces a redacted image, and so on. The platform must not alter the source format unless explicitly requested.

In addition to the format-preserving primary output, the platform should produce supplementary outputs that serve downstream workflows:

- **Redaction metadata (JSON)**: A structured manifest describing every redaction applied: entity type, location, triggering rule, confidence score, and reviewer disposition. This metadata enables programmatic consumption of redaction results by audit systems, analytics pipelines, and downstream integrations.
- **Masked structured data (CSV/JSON)**: For tabular or structured inputs, a masked variant in which sensitive cell values are replaced according to the active masking strategy, suitable for analytics or data science consumption.
- **Anonymized datasets**: Fully de-identified exports intended for secondary use (model training, statistical analysis) where no re-identification pathway should exist.

### 4.3 Masking Strategies

Multiple masking strategies should be available, selected according to the use case:

- **Tokenization and pseudonymization**: Replacement of sensitive values with consistent tokens that preserve referential integrity across documents.
- **Reversible masking**: Vault-based masking where original values can be recovered by authorized parties through a secure key exchange.
- **De-identification with re-linking key**: Removal of direct identifiers with a separately stored mapping that enables re-identification under controlled conditions.

## 5. Content Model

The internal content representation separates raw data from descriptive metadata. This separation is fundamental to reliable format detection and pipeline execution.

### 5.1 Content Data and Metadata

Each piece of ingested content is represented as a `Content` bundle consisting of two parts:

- **Content data**: The raw bytes and a unique source identifier. Content data carries no descriptive information: it is opaque binary or text.
- **Content metadata**: All descriptive attributes that accompany the content: the caller-supplied MIME type, the auto-detected MIME type (from magic-byte signatures), the original filename, the source path, and an extensible key-value map for arbitrary metadata.

This separation ensures that descriptive information is never lost during storage and retrieval. The registry persists data and metadata in separate storage keyspaces; when content is reconstructed for pipeline execution, both halves are reunited into a complete bundle.

### 5.2 Format Detection

Format detection determines the document type (text, image, audio, PDF, spreadsheet, etc.) and drives decoder selection. Detection evaluates multiple signals in priority order:

1. **Caller-supplied MIME type** — highest priority. Set explicitly at upload time (e.g. from an HTTP `Content-Type` header). This is the only way to identify text formats that have no magic bytes.
2. **Magic-byte detection** — automatic detection from binary signatures in the raw content. Reliable for binary formats (PNG, JPEG, WAV, PDF) but returns nothing for plain text.
3. **Filename extension** — lowest priority. Used as a fallback when MIME type and magic bytes are inconclusive. When the MIME type identifies a broad category (e.g. `text/plain`) and the extension provides a more specific variant (e.g. `.log`), the extension refines the result.

Format detection occurs at decode time, not at upload. Metadata is persisted at upload so that all detection signals are available when the content is later processed.

### 5.3 Metadata Through the Pipeline

Once content is decoded into a document, the original metadata is preserved on the pipeline envelope and flows through every processing stage — detection, fusion, policy evaluation, redaction, and export. This enables operations to make context-aware decisions based on the original filename, MIME type, or custom metadata without re-reading the raw content.

## 6. Validation and Error Handling

Ingestion must account for real-world content that is malformed, incomplete, or unsupported.

### 6.1 Input Validation

Before processing begins, the platform must validate that submitted content meets minimum requirements: supported file format, non-zero size, and absence of corruption indicators. Invalid submissions must be rejected with actionable error messages that identify the specific validation failure.

### 6.2 Partial Extraction

When a document is partially parseable — a multi-page PDF with a corrupt page, an audio file with a damaged segment, or an image with an unreadable region — the platform should extract what it can and flag the remainder as incomplete. Partial extraction results must be clearly annotated so that downstream detection operates only on successfully extracted content.

### 6.3 Error Reporting

Every ingestion failure must produce a structured error record that includes the content identifier, the failure type (unsupported format, corrupt data, extraction timeout, codec unavailable), and the processing stage at which the failure occurred. These records must be available through the same audit infrastructure described in [COMPLIANCE.md](COMPLIANCE.md).
