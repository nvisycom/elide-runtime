# elide-governance

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Wire schema for Nvisy policies: authored redaction governance.

## Overview

A policy declares named label scopes (what to detect), rules (what
to do), and an optional fallback (what happens to anything no rule
claimed). Each request submits policies in precedence order; the
engine walks them and, for each policy, walks its rules in order and
runs the first matching rule's action.

Scopes let a policy detect more than its rules act on: scope a whole
regulatory category, write rules for the labels needing special
treatment, and let the fallback sweep the rest. Policies carry
per-modality redaction operators (erase, mask, replace, hash,
encrypt, blur, pixelate, ...) and entity-match predicates, which may
target a scope by name.

Sibling to elide-wire, which carries the plan and file schemas.
Consumers depend on this crate directly.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
