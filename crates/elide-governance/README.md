# elide-governance

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Wire schema for Nvisy policies: authored redaction governance.

## Overview

A `Policy` is authored vocabulary that tells the engine *what to do*
when detection fires. Each request submits a `Vec<Policy>` in
precedence order; the engine walks them and, for each policy, walks
its rules in order and runs the first matching rule's action.
Policies carry per-modality redaction operators (erase, mask,
replace, hash, encrypt, blur, pixelate, ...), entity-match
predicates, and label groups the predicates reference by name.

Sibling to `elide-wire`, which re-exports this crate alongside its
own plan and file types.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
