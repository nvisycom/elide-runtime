# Contributing

Thank you for your interest in contributing to Nvisy Runtime.

## Requirements

- Rust 1.85+ (stable)
- Cargo
- [uv](https://docs.astral.sh/uv/), for the Python inference services
- Docker, for the service images and the adapter's end-to-end tests

## Setup

```bash
git clone https://github.com/nvisycom/elide-runtime.git
cd elide-runtime
cargo build
```

## Development

Run all CI checks locally before submitting a pull request:

```bash
cargo fmt --check     # formatting
cargo clippy          # lints
cargo test            # tests
```

To auto-fix formatting:

```bash
cargo fmt
```

### Inference services

The Python services in `packages/` are separate uv projects, one per service,
each with its own `uv.lock` and `.venv`; `elide-core` is the library they share.
The root `pyproject.toml` holds only the tooling: ruff, pytest for `elide-core`,
the contract generator, and the end-to-end harness's dependencies. Run every
command from the repository root:

```bash
make sync             # the tooling, plus each service's own .venv
make services-lint    # ruff over everything
make services-test    # elide-core, then each service in its own environment
make lock             # relock everything after editing a pyproject.toml
```

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make changes with tests
4. Run `cargo fmt --check && cargo clippy && cargo test` to verify all checks pass
5. Submit a pull request

## Project Structure

The monorepo uses Cargo workspaces. All crates live under `crates/`, and the
Python inference services under `packages/`. See the
[CHANGELOG](CHANGELOG.md) for a description of each crate.

## Releasing

A `v*` tag publishes the service images and creates the GitHub release.
`docker/compose.yaml` and the images are one versioned unit, so the commit you
tag must already name its own version:

1. Set every `${ELIDE_VERSION:-...}` default in `docker/compose.yaml`, and
   `ELIDE_VERSION` in `docker/.env.example`, to the version being released.
2. Merge, then tag that commit, e.g. `git tag v0.1.0-rc.7`, and push the tag.

Until the tag's images are published, a plain `docker compose up` on `main`
cannot pull; `docker/compose.dev.yaml` runs locally built images in the
meantime. Both release workflows run `scripts/check_compose_version.sh`, which
fails the release if step 1 was missed.

## Security

- Never commit secrets or API keys
- Use environment variables for configuration
- Validate all external inputs

## License

By contributing, you agree your contributions will be licensed under the
Apache License 2.0.
