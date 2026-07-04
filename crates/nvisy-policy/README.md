# nvisy-policy

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/rs-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/rs-build.yml)

Wire schema for Nvisy policies: authored redaction governance.

## Overview

A `Policy` is authored vocabulary that tells the engine *what to do*
when detection fires. Each request submits a `Vec<Policy>` in
precedence order; the engine walks them and, for each policy whose
predicate holds against the document, walks its rules in order and
runs the first matching rule's action. Policies carry per-modality
redaction operators (erase, mask, replace, hash, encrypt, blur,
pixelate, ...), predicates (document / entity match), retention
schedules, audit envelopes, and suppression rules.

Sibling to `nvisy-context` (reference data telling detection what to
look for) and `nvisy-schema` (umbrella re-exporting both).

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
