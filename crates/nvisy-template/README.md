# nvisy-template

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/runtime-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/runtime-build.yml)

Ready-to-run Nvisy policy templates for common regulatory postures.

## Overview

The `PolicyTemplate` enum names every regulatory posture this
crate ships; `PolicyTemplate::HipaaSafeHarbor.build()` materialises
the picked variant into a `Template` value carrying its
`PolicyDefinition` (with inline `LabelGroup`s) matched to how the
engine consumes it. Callers hand `template.policy` (as a
one-element slice via `std::slice::from_ref`) to `Engine::analyze`
/ `Engine::anonymize`, or compose several templates' policies into
one slice when they want more than one regulatory posture per
request.

Templates ship with their own semver-tracked `version` and the
`effective_date` of the regulatory text they encode, so customers
can pin to a snapshot when compliance requires it and audit which
version drove a given run.

Every template also carries a machine `id` (snake_case, stable)
distinct from its display `name` — mirroring how elide's `Label`
splits identity from display.

## What's covered

Five templates across four regulatory postures:

- **HIPAA Safe Harbor** — 18-identifier de-identification per
  §164.514(b)(2). `id = "hipaa_safe_harbor"`.
- **GDPR Article 9** — special-category personal data.
  `id = "gdpr_article_9"`.
- **PCI DSS §3.5.1** — Primary Account Number (PAN) render
  posture. Ships two templates so callers pick per their control
  choice: `id = "pci_dss_pan_truncate"` and
  `id = "pci_dss_pan_hmac"`.
- **CCPA** — Cal. Civ. Code §1798.140 personal-information
  categories. `id = "ccpa"`.

## Discovery

`TemplateCatalog::builtin()` returns a `(id, version)`-keyed
registry of every shipped template. Look up by `id + version`
via `catalog.get(id, version)`, or `catalog.latest(id)` for the
newest version. Serialises as a flat JSON array for discovery
endpoints.

Every template is a starting point. Customers override the
per-label operator or fallback where their internal posture
diverges — the returned `PolicyDefinition` is plain data.
