# nvisy-pattern

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/build.yml)

Built-in patterns, dictionaries, and validators for PII/PHI detection in the
Nvisy runtime.

## Overview

A pre-compiled pattern engine for PII/PHI detection. Each scan runs
regex (`RegexSet`-prefiltered), dictionary lookup (Aho-Corasick),
and deny-list injection. Built-in patterns and dictionaries live as
JSON under `assets/` and are embedded at compile time.

Per-scan inputs (allow / deny lists, context-keyword hints,
caller-supplied ad-hoc patterns) flow through `PatternContext` without
rebuilding the engine. Regex patterns can opt into post-match
validation by name (e.g. `"luhn"`, `"ssn"`, `"iban"`).

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
