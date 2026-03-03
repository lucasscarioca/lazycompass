# Contributing

Thanks for helping build LazyCompass. This file covers development setup and workflow.

## Prerequisites

- Rust (LTS toolchain, edition 2024)
- Docker (optional, for the local playground)

## Workspace Layout

- `lazycompass` (binary) in `lazycompass-cli/`
- `lazycompass-tui`: TUI library
- `lazycompass-core`: shared domain models
- `lazycompass-output`: shared result rendering
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

Manual QA guide: [dev/qa/README.md](./dev/qa/README.md)

Start:

```bash
docker compose -f dev/mongodb/docker-compose.yml up -d
```

Reset data:

```bash
docker compose -f dev/mongodb/docker-compose.yml down -v
docker compose -f dev/mongodb/docker-compose.yml up -d
```

## Manual Checks Before Commit/Push

- Before commit: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`
- Before push (always, including direct pushes to `main`): `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo build --workspace`, `cargo test --workspace`

## Git Workflow

- Maintainer workflow: direct commits/pushes to `main`
- External contributions: issues and PRs are welcome, but expect the maintainer to keep working directly on `main`
- Rule: always run full local checks before pushing

## Style and Guidelines

- Follow [AGENTS.md](./AGENTS.md) for code style and architecture rules
- Keep changes small and consistent with existing patterns
- Add or update tests when behavior changes

## Docs

- [CONFIGURATION.md](./CONFIGURATION.md): config schema and defaults
- [QUERY_FORMAT.md](./QUERY_FORMAT.md): saved query/aggregation schemas
- [VERSIONING.md](./VERSIONING.md): SemVer policy
- [RELEASE.md](./RELEASE.md): release checklist
- [CHANGELOG.md](./CHANGELOG.md): release notes
- [dev/qa/README.md](./dev/qa/README.md): manual playground validation

## Releases

- Update [CHANGELOG.md](./CHANGELOG.md) for every release.
- Follow [RELEASE.md](./RELEASE.md) for the release checklist.
