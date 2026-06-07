# nvisy-fake

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Locale-aware fake-data anonymizer for the Nvisy runtime.

## Overview

`FakeText` is a text-modality `Anonymizer` that swaps a detected entity
for a plausible fake value drawn from the [`fake`](https://docs.rs/fake)
crate's locale tables. The locale is selected per-entity from the
entity's BCP-47 `language` field; entities without a language tag
fall back to the operator's configured default.

RNG state is derived per-call from the entity UUID (optionally salted
with a workspace seed), so repeat runs over the same document produce
the same fake values. Entity kinds outside the core PII set
(person name, contact info, address, payment card, IBAN, currency,
date of birth, age) fall through to a `[{entity_kind}]` placeholder.

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
