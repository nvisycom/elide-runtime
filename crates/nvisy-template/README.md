# nvisy-template

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/runtime-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/runtime-build.yml)

Ready-to-run Nvisy policy templates for common regulatory postures.

## Overview

Each template returns a self-contained `Template` value: a set of
`PolicyDefinition`s plus the `LabelGroup`s those policies reference,
matched to how the engine consumes them. Callers hand the template
straight to `Engine::analyze` / `Engine::anonymize`; no assembly
required.

Templates ship with their own semver-tracked `version` and the
`effective_date` of the regulatory text they encode, so customers
can pin to a snapshot when compliance requires it and audit which
version drove a given run.

## What's covered

- **HIPAA Safe Harbor** — 18-identifier de-identification per
  §164.514(b)(2).
- **GDPR Article 9** — special-category personal data.
- **PCI DSS §3.5.1** — Primary Account Number (PAN) render posture.
  Ships two variants (truncate / keyed hash) so callers pick per
  their control choice.
- **CCPA** — Cal. Civ. Code §1798.140 personal-information
  categories.

Every template is a starting point. Customers override the
per-label operator or fallback where their internal posture
diverges — the returned `PolicyDefinition` is plain data.
