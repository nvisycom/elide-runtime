# Makefile for the Nvisy runtime workspace (Rust crates + Python
# BentoML packages).

# Default to a single recipe shell so a failure inside a piped
# command (e.g. server panics under `tee`) is reported by make.
.SHELLFLAGS := -eu -o pipefail -c
SHELL       := /bin/bash

# Timestamped log line, tagged with the running target. Use as `$(call log,msg)`.
define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

# Python services, keyed by package suffix (packages/nvisy-<suffix>).
PY_SERVICES := ocr vl ner


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

.PHONY: rust-ci
rust-ci: lint test doc deny ## Rust: full CI matrix.
	@$(call log,Rust CI passed.)


# ─── Python ────────────────────────────────────────────────────

.PHONY: py-sync
py-sync: ## Python: install BentoML and all workspace dependencies into the venv.
	@$(call log,Syncing workspace...)
	@uv sync --all-packages
	@$(call log,Workspace ready.)

.PHONY: py-lint
py-lint: ## Python: ruff check + format check.
	@$(call log,Running ruff check...)
	@uv run ruff check .
	@$(call log,Running format check...)
	@uv run ruff format --check .
	@$(call log,Lint passed.)

.PHONY: py-fmt
py-fmt: ## Python: auto-format with ruff.
	@$(call log,Formatting...)
	@uv run ruff format .
	@uv run ruff check --fix .
	@$(call log,Formatted.)

.PHONY: py-test
py-test: ## Python: run the test suite.
	@$(call log,Running tests...)
	@uv run pytest

.PHONY: py-generate
py-generate: ## Python: regenerate per-service requirements.
	@$(call log,Regenerating service requirements...)
	@uv run python scripts/gen_requirements.py
	@$(call log,Generated.)

.PHONY: py-check
py-check: ## Python: fail if generated per-service requirements are stale (CI parity).
	@$(call log,Checking service requirements...)
	@uv run python scripts/gen_requirements.py --check
	@$(call log,Generated artifacts up to date.)

.PHONY: py-serve-ocr
py-serve-ocr: ## Python: serve the OCR (docTR) service locally with reload.
	@$(call log,Serving nvisy-ocr...)
	@uv run bentoml serve nvisy_ocr.service:OcrService --reload

.PHONY: py-serve-vl
py-serve-vl: ## Python: serve the vision-language OCR (PaddleOCR-VL) service locally with reload.
	@$(call log,Serving nvisy-vl...)
	@uv run bentoml serve nvisy_vl.service:OcrVlService --reload

.PHONY: py-serve-ner
py-serve-ner: ## Python: serve the NER (GLiNER) service locally with reload.
	@$(call log,Serving nvisy-ner...)
	@uv run bentoml serve nvisy_ner.service:NerService --reload

.PHONY: py-build
py-build: ## Python: build all Bentos from their bentofiles.
	@for s in $(PY_SERVICES); do \
		$(call log,Building nvisy-$$s...); \
		uv run bentoml build -f packages/nvisy-$$s/bentofile.yaml . ; \
	done
	@$(call log,Bentos built.)

.PHONY: py-containerize
py-containerize: ## Python: build + containerize all Bentos into local Docker images.
	@for s in $(PY_SERVICES); do \
		$(call log,Containerizing nvisy-$$s...); \
		uv run bentoml build -f packages/nvisy-$$s/bentofile.yaml --containerize . ; \
	done
	@$(call log,Images built.)

.PHONY: py-ci
py-ci: py-lint py-check py-test ## Python: full CI matrix.
	@$(call log,Python CI passed.)


# ─── Combined ──────────────────────────────────────────────────

.PHONY: ci
ci: rust-ci py-ci ## Run the full CI matrix (Rust + Python).
	@$(call log,All CI checks passed.)


# `help` parses the `## …` doc comment after each target name and
# prints `target — description`. Keeping help auto-generated from
# the targets themselves means new targets don't need a manual
# entry to show up.
.PHONY: help
help:  ## Show this help.
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_.-]+:.*## / { printf "  %-16s  %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
