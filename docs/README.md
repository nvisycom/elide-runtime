# Documentation

Published documentation lives at [docs.nvisy.com](https://docs.nvisy.com). What
is here is the material that belongs beside the code rather than on the site.

- [**inference.md**](inference.md) — the HTTP contract the model services
  speak, for anyone running their own inference behind it.
- [**design/**](design) — why a given model or approach was chosen, and what
  was ruled out. One file per recognition layer: [ner](design/ner.md),
  [ocr](design/ocr.md).

Operational setup for the shipped services — compose, caches, publishing — is
in [`docker/`](../docker/README.md).
