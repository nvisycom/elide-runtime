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

# The BentoML services, by modality.
SERVICES := ner ocr stt

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

# Depends on `requirements`: a service module calls
# `Image.requirements_file(...)` at import, which resolves the path
# eagerly, so pytest fails at collection without the exports.
.PHONY: services-test
services-test: requirements ## Run the service test suite.
	@$(call log,Running tests...)
	@uv run pytest

.PHONY: requirements
requirements: ## Export each service's locked dependencies from uv.lock.
	@$(call log,Regenerating service requirements...)
	@uv run python scripts/gen_requirements.py

.PHONY: services
services: requirements ## Build every Bento from its bentofile.
	@for s in $(SERVICES); do \
		$(call log,Building elide-$$s...); \
		(cd packages/elide-$$s && uv run bentoml build); \
	done

# Built from the repo root with `-f`, never from the package directory:
# `Image.requirements_file()` resolves its path against the working directory,
# so a build started elsewhere cannot find the exported requirements.
#
# Two steps rather than `build --containerize`, which accepts no tag. `build`
# tags the bento with its content hash, so `-o tag` captures that (it prints
# the tag prefixed with `__tag__:`, hence the sed) to hand to
# `containerize -t`, which gives the predictable name
# docker/compose.dev.yaml refers to.
#
# `set -e` so a failed build fails the target instead of being swallowed by
# the loop.
.PHONY: services-image
services-image: requirements ## Build and containerize every Bento into a local Docker image.
	@set -e; for s in $(SERVICES); do \
		$(call log,Containerizing elide-$$s...); \
		bento=$$(uv run bentoml build -f packages/elide-$$s/bentofile.yaml -o tag . \
		          | sed 's/^__tag__://'); \
		uv run bentoml containerize "$$bento" -t "elide-$$s:latest"; \
	done

# Each depends on `requirements` for the same reason as
# `services-test`: `bentoml serve` imports the service module.
.PHONY: serve-ner
serve-ner: requirements ## Serve the NER service locally with reload.
	@uv run bentoml serve elide_ner.service:NerService --reload

.PHONY: serve-ocr
serve-ocr: requirements ## Serve the OCR service locally with reload.
	@uv run bentoml serve elide_ocr.service:OcrService --reload

.PHONY: serve-stt
serve-stt: requirements ## Serve the speech-to-text service locally with reload.
	@uv run bentoml serve elide_stt.service:SttService --reload

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
