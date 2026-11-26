# elide-review

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/elide-runtime/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/elide-runtime/actions/workflows/build.yml)

Reviewer edits over an elide report: add, retag, suppress, and what
happens to hidden content.

## Overview

An analyzed document goes to a human before it is redacted, and what
they decide lands here. An add records a detection recognition
missed: not a judgement about a detection but one in its own right,
sourced from a person. A retag and a suppress each name an existing
entity, correcting what it is or leaving it alone.

Edits are a list rather than one decision per entity, because they
feed independent channels: a retag corrects what a detection *is*,
while a suppress decides what happens to it. Both at once is a
reviewer doing two legitimate things. Two answers to the same
question are a contradiction, and validation rejects them instead of
letting one silently win.

Hidden content (metadata fields, comments, their authors, tracked
changes) is decided separately, by id: remove an item that would be
kept, keep one that would be removed, or remove them all. Each
decision is recorded on the report's audit trail, and the latest
about an item wins. The plan of what a pass removes, which the
reviewer previews and the anonymizer runs, lives here too.

An edit says what was decided and why, not who decided it. The host
names the reviewer when it applies the edits, from however it
authenticated them, so a request body cannot claim to be someone
else's.

The crate is split so the data half stands alone. Edit and EditSet
deserialize and validate with no engine in sight, which is what an
HTTP layer needs when it accepts a reviewer's edits before it has
anything to apply them to. Applying is the other half: the edits
become one elide review of the report, which elide checks whole
before recording any of it.

## Project

- **Changelog**: [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history
- **Contributing**: [CONTRIBUTING.md](../../CONTRIBUTING.md) for setup, local CI targets, and the pull-request process
- **License**: Apache 2.0, see [LICENSE](../../LICENSE)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/elide-runtime/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
