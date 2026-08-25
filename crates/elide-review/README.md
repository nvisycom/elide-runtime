# elide-review

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Reviewer edits over an elide report: add, retag, suppress, override.

## Overview

An analyzed document goes to a human before it is redacted, and what
they decide lands here. Four operations, in two groups. An add
records a detection recognition missed — not a judgement about a
detection but one in its own right, sourced from a person. A retag,
a suppress, and an operator override each name an existing entity and
change what the policy set would have done to it.

Edits are a list rather than one decision per entity, because they
feed independent channels: a retag corrects what a detection *is*,
while a suppress or an override decides what happens to it. Both at
once is a reviewer doing two legitimate things. Two answers to the
same question are a contradiction, and validation rejects them
instead of letting one silently win.

The crate is split so the data half stands alone. Edit and EditSet
deserialize and validate with no engine in sight, which is what an
HTTP layer needs when it accepts a reviewer's edits before it has
anything to apply them to. Applying is the other half: three of the
four operations land on the report, while the operator override goes
onto the anonymizer instead, because elide re-resolves operators from
live policy at apply time and has no per-entity override of its own.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
