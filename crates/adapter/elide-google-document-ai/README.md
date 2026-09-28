# elide-google-document-ai

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/rust-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/rust-build.yml)

Google Cloud Document AI-backed OCR backend for elide.

## Overview

An `OcrBackend` over [Document AI](https://cloud.google.com/document-ai)'s
Enterprise Document OCR processor. A sibling to
[`elide-bentoml`](../elide-bentoml), not a part of it: both implement the
same trait, so a deployment picks one without either knowing about the
other. The self-hosted alternative is
[`bento-doctr`](../../packages/bento-doctr).

Transport is Google's official
[`google-cloud-documentai-v1`](https://crates.io/crates/google-cloud-documentai-v1)
crate, re-exported as `elide_google_document_ai::document_ai` so
configuring a client needs no second dependency. `DocumentAiOcr::new`
takes the client and the full processor resource name,
`projects/{p}/locations/{l}/processors/{id}`.

`process_document` is synchronous — one call returns the document, which
matches the `OcrBackend` contract's single `await`. Use it rather than
`batch_process_documents`: Google documents the online path as processed
in memory and *"not persisted to disk"*, which the batch path does not
promise.

Regions are emitted at **token** level, Document AI's word equivalent and
the finest granularity it reports. Blocks, paragraphs and lines are
discarded: elide's `Layout` is a flat list, and tokens are where a
redaction pipeline wants its spans.

Geometry comes from the response's pixel `vertices` rather than its
`normalized_vertices`, so no page dimensions are needed to convert.
Confidence arrives already in `0.0..=1.0`. A layout with no pixel
vertices is dropped rather than placed at the origin, which would redact
the wrong part of the image.

Text is carried once on the document and addressed by **byte** offset, so
a span is sliced out of it. An anchor that runs past the text, inverts, or
lands inside a multi-byte character is skipped rather than clamped — a
mis-sliced span redacts the wrong bytes, and the crate handles non-ASCII
documents.

## Before pointing this at production documents

**This sends document images to a third party**, which for a redaction
pipeline is the un-redacted original. `bento-doctr` runs locally with no
egress.

Google's terms are the best-documented of the hosted OCR providers, and
they are why this is the one to reach for first where either fits
([`elide-aws-textract`](../elide-aws-textract) is the other):

- online processing is *"processed in memory ... not persisted to disk"*
- *"we never use customer data to train our Document AI models"*
- Document AI is covered by the HIPAA BAA

Two things still need setting deliberately:

- **Use the online path.** `batch_process_documents` writes to Cloud
  Storage and carries none of the in-memory guarantee.
- **Set the regional endpoint explicitly.** The global default gives no
  data-residency guarantee.

Confirm the terms directly rather than from this README, which was written
in September 2026 and will age.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
