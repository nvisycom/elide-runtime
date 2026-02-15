# Makefile for the Nvisy runtime monorepo.

ifneq (,$(wildcard ./.env))
    include .env
    export
endif

export PYO3_USE_ABI3_FORWARD_COMPATIBILITY := 1

define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

.PHONY: dev
dev: ## Starts cargo-watch for the server binary.
	@cargo watch -x 'run -p nvisy-server'

.PHONY: build
build: ## Builds all crates in release mode.
	@$(call log,Building workspace...)
	@cargo build --workspace --release
	@$(call log,Build complete.)

.PHONY: check
check: ## Runs cargo check on all crates.
	@cargo check --workspace

.PHONY: test
test: ## Runs all tests.
	@cargo test --workspace

.PHONY: lint
lint: ## Runs clippy and format check.
	@$(call log,Running format check...)
	@cargo fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --workspace -- -D warnings
	@$(call log,Lint passed.)

.PHONY: fmt
fmt: ## Formats all Rust code.
	@cargo fmt --all

.PHONY: ci
ci: lint check test build ## Runs all CI checks locally.
	@$(call log,All CI checks passed!)

.PHONY: clean
clean: ## Removes build artifacts.
	@$(call log,Cleaning build artifacts...)
	@cargo clean
	@$(call log,Clean complete.)

.PHONY: docker
docker: ## Builds the Docker image.
	@$(call log,Building Docker image...)
	@docker build -f docker/Dockerfile -t nvisy-runtime .
	@$(call log,Docker image built.)

.PHONY: help
help: ## Shows this help message.
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'
