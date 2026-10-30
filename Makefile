# Makefile for the Nvisy runtime: the Rust workspace in `crates/`
# and the Python model services in `packages/`.
#
# Targets are named for what they act on, not which toolchain runs
# them: `lint`/`test`/`doc` cover the crates, `services-*` the
# packages.

# Default to a single recipe shell so a failure inside a piped
# command (e.g. server panics under `tee`) is reported by make.
# GNU make before 3.82, which macOS still ships, ignores .SHELLFLAGS, so the
# loops below also `set -e` themselves.
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

# Each service is its own uv project with its own lock and .venv, so every
# target runs a command in the environment of the project it acts on:
# `uv run --project packages/elide-<s>`. Always from the repository root, never
# from inside a package: `Image.requirements_file()` resolves its path against
# the working directory when a service module is imported.
.PHONY: sync
sync: ## Install the tooling and every service's dependencies into their venvs.
	@$(call log,Syncing tooling...)
	@uv sync
	@set -e; for s in $(SERVICES); do \
		$(call log,Syncing elide-$$s...); \
		uv sync --project packages/elide-$$s; \
	done

.PHONY: lock
lock: ## Relock the tooling and every service after a manifest change.
	@uv lock
	@set -e; for s in $(SERVICES); do uv lock --project packages/elide-$$s; done

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
services-test: requirements ## Run elide-core's tests, then each service's.
	@$(call log,Testing elide-core...)
	@uv run pytest
	@set -e; for s in $(SERVICES); do \
		$(call log,Testing elide-$$s...); \
		uv run --project packages/elide-$$s pytest packages/elide-$$s; \
	done

.PHONY: requirements
requirements: ## Export each service's locked dependencies from its uv.lock.
	@$(call log,Regenerating service requirements...)
	@python3 scripts/gen_requirements.py $(addprefix elide-,$(SERVICES))

# Two steps rather than `build --containerize`, which accepts no tag. `build`
# tags the bento with its content hash, so `-o tag` captures that (it prints
# the tag prefixed with `__tag__:`, which is stripped) to hand to
# `containerize -t`, which gives the predictable name
# docker/compose.dev.yaml refers to. The tag is captured without a pipe, so a
# failed build fails the assignment rather than hiding behind the pipe's last
# command.
#
# Builds for the host architecture only, which is what a developer wants: the
# image runs natively, and cross-building under emulation is slow enough to be
# useless for these (an emulated PyTorch import does not finish loading). The
# published multi-architecture manifests are assembled in CI from native
# runners per architecture, not here.
.PHONY: services
services: requirements ## Build every Bento from its bentofile.
	@set -e; for s in $(SERVICES); do \
		$(call log,Building elide-$$s...); \
		uv run --project packages/elide-$$s \
			bentoml build -f packages/elide-$$s/bentofile.yaml .; \
	done

.PHONY: services-image
services-image: requirements ## Build and containerize every Bento into a local Docker image.
	@set -e; for s in $(SERVICES); do \
		$(call log,Containerizing elide-$$s...); \
		bento=$$(uv run --project packages/elide-$$s \
			bentoml build -f packages/elide-$$s/bentofile.yaml -o tag .); \
		bento=$${bento#__tag__:}; \
		uv run --project packages/elide-$$s \
			bentoml containerize "$$bento" -t "elide-$$s:latest"; \
	done

# Each depends on `requirements` for the same reason as
# `services-test`: `bentoml serve` imports the service module.
.PHONY: serve-ner
serve-ner: requirements ## Serve the NER service locally with reload.
	@uv run --project packages/elide-ner bentoml serve elide_ner.service:NerService --reload

.PHONY: serve-ocr
serve-ocr: requirements ## Serve the OCR service locally with reload.
	@uv run --project packages/elide-ocr bentoml serve elide_ocr.service:OcrService --reload

.PHONY: serve-stt
serve-stt: requirements ## Serve the speech-to-text service locally with reload.
	@uv run --project packages/elide-stt bentoml serve elide_stt.service:SttService --reload

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
