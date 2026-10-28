# Documentation

Published documentation lives at [docs.nvisy.com](https://docs.nvisy.com). What
is here is the material that belongs beside the code rather than on the site.

- [**inference.md**](inference.md) — the HTTP contract the model services
  speak, for anyone running their own inference behind it.
- [**design/**](design/README.md) — shared service architecture and the decisions
  behind each inference path: [NER](design/ner.md), [OCR](design/ocr.md), and
  [STT](design/stt.md).

Operational setup for the shipped services — compose, caches, publishing — is
in [`docker/`](../docker/README.md).
