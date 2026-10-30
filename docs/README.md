# Documentation

Published documentation lives at [docs.nvisy.com](https://docs.nvisy.com). What
is here is the material that belongs beside the code rather than on the site.

- [**inference.md**](inference.md) — the HTTP contract the model services
  speak, for anyone running their own inference behind it.
- [**design/**](design/README.md) — how to choose each service's model: what it
  must do, the options considered, why the default won, and where it falls
  short ([NER](design/ner.md), [OCR](design/ocr.md), [STT](design/stt.md)),
  followed by how the service runs it.

Operational setup for the shipped services — compose, caches, publishing — is
in [`docker/`](../docker/README.md).
