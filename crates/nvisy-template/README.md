# nvisy-template

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/runtime/runtime-build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/runtime/actions/workflows/runtime-build.yml)

Ready-to-run Nvisy policy templates for common regulatory postures.

## Overview

Each template packages a regulatory posture as engine-ready data:
the labels the regulation targets, the operator dispatched at
each label, and the identity metadata (machine id, display name,
semver version, and the effective date of the regulatory text)
audits key on. Where a regulation permits more than one operator,
the template exposes a small options type naming the shipped
choices; where it doesn't, no options.

Templates are plain data. Callers submit them to the engine as-is,
or mutate the returned policy where their internal posture
diverges from the shipped default.

## What's covered

Four regulatory postures across seven shipped variants:

- **HIPAA §164.514 de-identification**. Ships all three methods
  the rule permits: Safe Harbor (fixed 18-identifier removal),
  Limited Data Set (narrower subtraction that keeps dates,
  ages, and coarse geography for research handoffs governed by
  a Data Use Agreement), and an Expert Determination scaffold
  (identity-preserving pseudonymization; requires a qualified
  statistician to attest that re-identification risk is "very
  small" before the output can be treated as de-identified).
- **GDPR Article 9** — special-category personal data.
- **PCI DSS §3.5.1** — Primary Account Number render posture.
  Ships both permitted approaches: truncation (BIN + last-four,
  no key material) and keyed HMAC-SHA-256 (preserves per-row
  identity for downstream joins and dedup; requires a key
  provider wired into the engine).
- **CCPA** — Cal. Civ. Code §1798.140 personal-information
  categories.

## Discovery

A built-in catalog registers every shipped template keyed by
`(id, version)`. Look up an exact template or the newest version
of a family, iterate the full set, or serialise the catalog for
a discovery endpoint — a customer transitioning between
regulatory revisions can hold multiple versions of the same
posture simultaneously and pin per document class.
