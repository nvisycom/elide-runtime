# elide-bento

Shared BentoML HTTP client wrapper for elide backends.

Per-modality backends (NER, OCR, …) live in their consuming crates
(`elide-ner`, `elide-ocr`) and pull this crate for the common HTTP
client, params validation, and error translation.
