# Makefile for the Nvisy runtime monorepo.

ifneq (,$(wildcard ./.env))
    include .env
    export
endif

export PYO3_USE_ABI3_FORWARD_COMPATIBILITY := 1

define log
printf "[%s] [MAKE] [$(MAKECMDGOALS)] $(1)\n" "$$(date '+%Y-%m-%d %H:%M:%S')"
endef

.PHONY: install-tools
install-tools: ## Installs CLI tools required for development.
	@$(call log,Checking cargo-watch...)
	@if ! command -v cargo-watch >/dev/null 2>&1; then \
		$(call log,Installing cargo-watch...); \
		cargo install cargo-watch --locked; \
		$(call log,cargo-watch installed.); \
	else \
		$(call log,cargo-watch already installed.); \
	fi

.PHONY: generate-env
generate-env: ## Copies .env.example to .env.
	@$(call log,Copying .env.example to .env...)
	@cp ./.env.example ./.env
	@$(call log,.env file created successfully.)

.PHONY: generate-config
generate-config: ## Copies Nvisy.example.toml to Nvisy.toml.
	@$(call log,Copying Nvisy.example.toml to Nvisy.toml...)
	@cp ./Nvisy.example.toml ./Nvisy.toml
	@$(call log,Nvisy.toml created successfully.)

.PHONY: install
install: install-tools generate-env generate-config ## Installs all dependencies and makes scripts executable.
	@chmod +x scripts/*.sh
	@$(call log,Installing PDFium...)
	@./scripts/install-pdfium.sh
	@$(call log,Setup complete.)

.PHONY: dev
dev: ## Starts cargo-watch for the server binary.
	@cargo watch -x 'run -p nvisy-server'

.PHONY: lint
lint: ## Runs clippy and format check.
	@$(call log,Running format check...)
	@cargo fmt --all -- --check
	@$(call log,Running clippy...)
	@cargo clippy --workspace -- -D warnings
	@$(call log,Lint passed.)

.PHONY: ci
ci: lint ## Runs all CI checks locally.
	@cargo check --workspace
	@cargo test --workspace
	@cargo build --workspace --release
	@$(call log,All CI checks passed!)

.PHONY: docker
docker: ## Builds the Docker image.
	@$(call log,Building Docker image...)
	@docker build -f docker/Dockerfile -t nvisy-runtime .
	@$(call log,Docker image built.)

.PHONY: help
help: ## Shows this help message.
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
