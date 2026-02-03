# Contributing

Thanks for helping build LazyCompass. This file covers development setup and workflow.

## Prerequisites

- Rust (LTS toolchain, edition 2024)
- Docker (optional, for the local playground)
- Lefthook (optional, for git hooks)

## Workspace Layout

- `lazycompass` (binary) in `lazycompass-cli/`
- `lazycompass-tui`: TUI library
- `lazycompass-core`: shared domain models
- `lazycompass-storage`: config paths + persistence
- `lazycompass-mongo`: MongoDB execution layer

## Build / Lint / Test

From repo root:

```bash
cargo build
cargo build -p lazycompass
```

Format:

```bash
cargo fmt
```

Lint:

```bash
cargo clippy --workspace
```

Test:

```bash
cargo test --workspace
```

Run a single test by name (any crate):

```bash
cargo test test_name
```

Run a single test in a specific crate:

```bash
cargo test -p lazycompass-core test_name
cargo test -p lazycompass-storage test_name
```

Run a single test in a specific module:

```bash
cargo test -p lazycompass-core module_name::test_name
```

## Local Playground

Start:

```bash
docker compose -f dev/mongodb/docker-compose.yml up -d
```

Reset data:

```bash
docker compose -f dev/mongodb/docker-compose.yml down -v
docker compose -f dev/mongodb/docker-compose.yml up -d
```

## Git Hooks (Optional)

This repo uses Lefthook for git hooks.

Install:

```bash
lefthook install
```

Hooks:

- Pre-commit: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`
- Pre-push: `cargo build --workspace`, `cargo test --workspace`

## Style and Guidelines

- Follow `AGENTS.md` for code style and architecture rules
- Keep changes small and consistent with existing patterns
- Add or update tests when behavior changes

## Docs

- `SPEC.md`: vision and roadmap
- `PLAYGROUND.md`: test data and setup
