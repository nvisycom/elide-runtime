# Makefile for the Nvisy runtime workspace.

define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

.PHONY: lint
lint: ## Format check + clippy.
	@$(call log,Running format check...)
	@cargo fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --workspace --all-targets -- -D warnings
	@$(call log,Lint passed.)

.PHONY: test
test: ## Run the workspace test suite.
	@cargo test --workspace

.PHONY: doc
doc: ## Build rustdoc for every crate with `-D warnings` (nightly, for --cfg docsrs).
	@RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo +nightly doc --workspace --no-deps

.PHONY: deny
deny: ## Run cargo-deny (advisories, bans, licenses, sources).
	@cargo deny check

.PHONY: ci
ci: lint test doc deny ## Run the full CI matrix locally.
	@$(call log,All CI checks passed.)

# `help` parses the `## …` doc comment after each target name and
# prints `target — description`. Keeping help auto-generated from
# the targets themselves means new targets don't need a manual
# entry to show up.
.PHONY: help
help:  ## Show this help.
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_.-]+:.*## / { printf "  %-14s  %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
