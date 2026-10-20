# Makefile for the Nvisy runtime: the Rust workspace in `crates/`
# and the Python model services in `packages/`.
#
# Targets are named for what they act on, not which toolchain runs
# them: `lint`/`test`/`doc` cover the crates, `services-*` the
# packages.

# Default to a single recipe shell so a failure inside a piped
# command (e.g. server panics under `tee`) is reported by make.
.SHELLFLAGS := -eu -o pipefail -c
SHELL       := /bin/bash

# The BentoML services, by their package suffix.
SERVICES := doctr paddleocr gliner2 whisper

# Timestamped log line, tagged with the running target. Use as `$(call log,msg)`.
define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef


# ─── Crates ────────────────────────────────────────────────────

.PHONY: lint
lint: ## Format check + clippy over the workspace.
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

.PHONY: machete
machete: ## Find dependencies declared but never used.
	@cargo machete

.PHONY: deny
deny: ## Run cargo-deny (advisories, bans, licenses, sources).
	@cargo deny check

.PHONY: ci
ci: lint test doc machete deny ## Run the crate CI matrix.
	@$(call log,Crate CI passed.)


# ─── Services ──────────────────────────────────────────────────

.PHONY: sync
sync: ## Install BentoML and every Python workspace dependency into the venv.
	@$(call log,Syncing workspace...)
	@uv sync --all-packages

.PHONY: services-lint
services-lint: ## ruff check + format check over the services.
	@$(call log,Running ruff check...)
	@uv run ruff check .
	@$(call log,Running ruff format check...)
	@uv run ruff format --check .

.PHONY: services-fmt
services-fmt: ## Auto-format the services with ruff.
	@$(call log,Formatting...)
	@uv run ruff format .

.PHONY: services-test
services-test: ## Run the service test suite.
	@$(call log,Running tests...)
	@uv run pytest

.PHONY: requirements
requirements: ## Export each service's locked dependencies from uv.lock.
	@$(call log,Regenerating service requirements...)
	@uv run python scripts/gen_requirements.py

.PHONY: services
services: requirements ## Build every Bento from its bentofile.
	@for s in $(SERVICES); do \
		$(call log,Building bento-$$s...); \
		(cd packages/bento-$$s && uv run bentoml build); \
	done

.PHONY: services-image
services-image: requirements ## Build and containerize every Bento into a local Docker image.
	@for s in $(SERVICES); do \
		$(call log,Containerizing bento-$$s...); \
		(cd packages/bento-$$s && uv run bentoml build --containerize); \
	done

.PHONY: serve-doctr
serve-doctr: ## Serve the OCR (docTR) service locally with reload.
	@uv run bentoml serve bento_doctr.service:OcrService --reload

.PHONY: serve-paddleocr
serve-paddleocr: ## Serve the vision-language OCR service locally with reload.
	@uv run bentoml serve bento_paddleocr.service:VlService --reload

.PHONY: serve-gliner2
serve-gliner2: ## Serve the NER service locally with reload.
	@uv run bentoml serve bento_gliner2.service:NerService --reload

.PHONY: serve-whisper
serve-whisper: ## Serve the speech-to-text service locally with reload.
	@uv run bentoml serve bento_whisper.service:SttService --reload

.PHONY: services-ci
services-ci: services-lint services-test ## Run the service CI matrix.
	@$(call log,Service CI passed.)

.PHONY: ci-all
ci-all: ci services-ci ## Run both CI matrices.
	@$(call log,All CI passed.)


# `help` parses the `## …` doc comment after each target name and
# prints `target  description`. Keeping help auto-generated from
# the targets themselves means new targets don't need a manual
# entry to show up.
.PHONY: help
help:  ## Show this help.
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_.-]+:.*## / { printf "  %-20s  %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
