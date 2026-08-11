# Makefile for the Nvisy runtime workspace (Rust crates).

# Default to a single recipe shell so a failure inside a piped
# command (e.g. server panics under `tee`) is reported by make.
.SHELLFLAGS := -eu -o pipefail -c
SHELL       := /bin/bash

# Timestamped log line, tagged with the running target. Use as `$(call log,msg)`.
define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef


# ─── Rust ──────────────────────────────────────────────────────

.PHONY: lint
lint: ## Rust: format check + clippy.
	@$(call log,Running format check...)
	@cargo fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --workspace --all-targets -- -D warnings
	@$(call log,Lint passed.)

.PHONY: test
test: ## Rust: run the workspace test suite.
	@cargo test --workspace

.PHONY: doc
doc: ## Rust: build rustdoc for every crate with `-D warnings` (nightly, for --cfg docsrs).
	@RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo +nightly doc --workspace --no-deps

.PHONY: deny
deny: ## Rust: run cargo-deny (advisories, bans, licenses, sources).
	@cargo deny check

.PHONY: ci
ci: lint test doc deny ## Run the full CI matrix.
	@$(call log,Rust CI passed.)


# `help` parses the `## …` doc comment after each target name and
# prints `target — description`. Keeping help auto-generated from
# the targets themselves means new targets don't need a manual
# entry to show up.
.PHONY: help
help:  ## Show this help.
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_.-]+:.*## / { printf "  %-16s  %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
