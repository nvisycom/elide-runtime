# elide-aws-textract

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-provider/rust-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-provider/actions/workflows/rust-build.yml)

AWS Textract-backed OCR backend for elide.

## Overview

An `OcrBackend` over [Textract](https://aws.amazon.com/textract)'s
`DetectDocumentText`. A sibling to [`elide-bentoml`](../elide-bentoml),
not a part of it: both implement the same trait, so a deployment picks
one without either knowing about the other. The self-hosted alternative
is [`bento-doctr`](../../packages/bento-doctr), and the other hosted one
is [`elide-google-document-ai`](../elide-google-document-ai).

Transport is the official
[`aws-sdk-textract`](https://crates.io/crates/aws-sdk-textract) crate,
re-exported as `elide_aws_textract::textract` so configuring a client
needs no second dependency. `TextractOcr::new` takes that client, which
carries the region, credentials and retry policy.

`DetectDocumentText` is used rather than `AnalyzeDocument`: this backend
wants text and geometry, and the analysis features (forms, tables,
queries) cost more per page for results elide's layout model has nowhere
to put. The synchronous call is the one that matches the `OcrBackend`
contract's single `await`; the async `StartDocumentTextDetection` path is
a job queue whose results land in S3.

Regions are emitted at `WORD` level. `LINE` and `PAGE` blocks are
discarded: elide's `Layout` is a flat list, and words are where a
redaction pipeline wants its spans.

Geometry arrives as **ratios** of the page, so it is denormalized against
the `dimensions` on the request — Textract never sees the pixel size, and
a wrong one silently redacts the wrong region. A box that is not finite,
is negative, or escapes the unit square is dropped rather than clamped.
Confidence arrives as `0.0..=100.0` and is divided by 100 before it
reaches elide, whose `Confidence` clamps rather than rescales — an
undivided 99.2 would saturate to a flat 1.0.

## Before pointing this at production documents

**This sends document images to a third party**, which for a redaction
pipeline is the un-redacted original. `bento-doctr` runs locally with no
egress.

Textract's default data posture is the weakest of the hosted OCR
providers, and it is why this backend is the second one rather than the
first. Out of the box, AWS may:

- use content processed by Textract for service improvement, including
  model training
- store that content outside the AWS region the request was made in

Both are opt-out, and the opt-out is an **AWS Organizations AI services
policy** — not a per-request flag, not a console checkbox on the account
using the API. It attaches at the organization root, an organizational
unit, or an individual account, and the effective policy is what those
combine to. Treat it as day-one work that happens before the first
production page, and verify the effective policy for the account actually
making these calls.

The service is HIPAA-eligible under the AWS BAA, and supports VPC
endpoints (AWS PrivateLink) to keep requests off the public internet.
Neither is on by default.

Confirm the terms directly rather than from this README, which was written
in September 2026 and will age.

## License

Apache 2.0 License, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-provider/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
