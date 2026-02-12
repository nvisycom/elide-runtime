# Ingestion & Transformation

## 1. Overview

The ingestion layer is responsible for accepting content from heterogeneous sources and normalizing it into a unified internal representation suitable for downstream detection and redaction. The transformation layer handles the inverse concern: producing redacted output in the appropriate format while preserving the structural integrity of the original document.

The quality of the ingestion layer is a critical success factor. Redaction platforms that cannot reliably parse and extract content from real-world documents — scanned forms, embedded tables, multi-speaker audio — will produce incomplete redaction results regardless of the sophistication of their detection models.

## 2. Supported Input Formats

The platform must support ingestion across the following modalities:

- **Documents**: PDF (native and scanned), DOCX, HTML, plain text
- **Images**: JPG, PNG, TIFF, and other common raster formats
- **Video**: Standard container formats with frame-level extraction
- **Audio**: WAV, MP3, and other common audio formats
- **Structured data**: CSV, JSON, and database connectors
- **Communications**: Email (with attachments), chat logs (Slack, Teams, WhatsApp exports)

## 3. Extraction Capabilities

Each modality requires specialized extraction techniques:

- **Optical character recognition (OCR)**: Layout-aware OCR that preserves spatial relationships between text regions, table cells, headers, and form fields.
- **Speech-to-text**: Transcription with speaker diarization, enabling attribution of spoken content to individual speakers.
- **Video frame extraction**: Decomposition of video streams into individual frames for visual analysis, with temporal alignment to audio tracks.
- **Document structure parsing**: Identification of semantic document elements — headings, paragraphs, tables, lists, and form fields — beyond raw text extraction.
- **Metadata extraction**: Capture of authorship, timestamps, geolocation, and other embedded metadata that may itself constitute sensitive information.

## 4. Transformation & Output

Following redaction, the transformation layer must produce output that meets downstream requirements while maintaining fidelity to the original format.

### 4.1 Format Preservation

Redacted output should preserve the structural characteristics of the source document. Tables must remain aligned, page layouts must be maintained, and non-redacted content must remain unaltered.

### 4.2 Export Formats

The platform should support export as:

- Redacted PDF with visual redaction markers
- Structured JSON with redaction metadata
- Masked CSV for tabular data
- Anonymized datasets for analytics consumption

### 4.3 Masking Strategies

Multiple masking strategies should be available, selected according to the use case:

- **Tokenization and pseudonymization**: Replacement of sensitive values with consistent tokens that preserve referential integrity across documents.
- **Reversible masking**: Vault-based masking where original values can be recovered by authorized parties through a secure key exchange.
- **De-identification with re-linking key**: Removal of direct identifiers with a separately stored mapping that enables re-identification under controlled conditions.

## 5. Validation and Error Handling

Ingestion must account for real-world content that is malformed, incomplete, or unsupported.

### 5.1 Input Validation

Before processing begins, the platform must validate that submitted content meets minimum requirements: supported file format, non-zero size, and absence of corruption indicators. Invalid submissions must be rejected with actionable error messages that identify the specific validation failure.

### 5.2 Partial Extraction

When a document is partially parseable — a multi-page PDF with a corrupt page, an audio file with a damaged segment, or an image with an unreadable region — the platform should extract what it can and flag the remainder as incomplete. Partial extraction results must be clearly annotated so that downstream detection operates only on successfully extracted content.

### 5.3 Error Reporting

Every ingestion failure must produce a structured error record that includes the content identifier, the failure type (unsupported format, corrupt data, extraction timeout, codec unavailable), and the processing stage at which the failure occurred. These records must be available through the same audit infrastructure described in [COMPLIANCE.md](COMPLIANCE.md).
