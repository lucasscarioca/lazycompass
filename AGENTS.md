# LazyCompass Agent Guidelines

This file guides agentic coding assistants working in this repo.

## Repo Overview

- Rust workspace with multiple crates
- MongoDB TUI + CLI client
- Config stored as TOML and saved specs stored as JSON, repo-committable
- Local playground via Docker for testing

Workspace crates:

- `lazycompass` (binary) in `lazycompass-cli/`
- `lazycompass-tui`: TUI library
- `lazycompass-core`: shared domain models
- `lazycompass-storage`: config paths + persistence
- `lazycompass-mongo`: MongoDB execution layer

Key docs:

- `SPEC.md`: source of truth for vision/requirements
- `PLAYGROUND.md`: local MongoDB testing instructions

## Build / Lint / Test Commands

Use Cargo at workspace root.

Build:

```
cargo build
cargo build -p lazycompass
```

Format:

```
cargo fmt
```

Lint:

```
cargo clippy --workspace
```

Test all:

```
cargo test --workspace
```

Run a single test by name (any crate):

```
cargo test test_name
```

Run a single test in a specific crate:

```
cargo test -p lazycompass-core test_name
cargo test -p lazycompass-storage test_name
```

Run a single test in a specific module:

```
cargo test -p lazycompass-core module_name::test_name
```

Run only library tests for a crate:

```
cargo test -p lazycompass-core --lib test_name
```

Run doc tests (if added later):

```
cargo test --doc
```

## Manual Checks Before Commit/Push

Run before commit:

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`

Run before push (always, including direct pushes to `main`):

- `cargo fmt --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo build --workspace`
- `cargo test --workspace`

## Playground (MongoDB via Docker)

Start:

```
docker compose -f dev/mongodb/docker-compose.yml up -d
```

Reset data:

```
docker compose -f dev/mongodb/docker-compose.yml down -v
docker compose -f dev/mongodb/docker-compose.yml up -d
```

## Config Conventions

- Global config: `~/.config/lazycompass/`
- Repo config: `.lazycompass/`
- Saved queries: `.lazycompass/queries/*.json`
- Saved aggregations: `.lazycompass/aggregations/*.json`
- Repo config overrides global; global is fallback

## Code Style Guidelines

Follow Rust 2024 edition idioms and keep changes minimal and consistent.

### Formatting

- Use `cargo fmt` (rustfmt) only; no custom formatting rules
- 4-space indentation from rustfmt defaults
- Keep line length reasonable; rustfmt handles wrapping

### Imports

- Prefer explicit imports over glob imports
- Group imports by crate: std, external, internal
- Use workspace dependencies in `Cargo.toml` when possible

### Naming

- Types/traits/enums: `PascalCase`
- Functions/vars/modules: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Prefer short, descriptive names; avoid abbreviations unless standard

### Types and APIs

- Favor concrete types over heavy generics unless necessary
- Use `Option<T>` for optional fields
- Use `Result<T, E>` for fallible operations
- Prefer `&str` and `String` appropriately (`&str` for inputs)
- Prefer `Path`/`PathBuf` for filesystem paths

### Error Handling

- Use `anyhow::Result` for binaries and top-level flows
- Use `thiserror` for library error enums (when added)
- No `unwrap()`/`expect()` in production paths
- Add context with `anyhow::Context` when failing IO or parsing

### Serialization and Config

- Use `serde` for config and persisted data
- Keep config TOML and saved spec JSON schemas stable; document changes in `SPEC.md`
- Query/aggregation `filter`/`pipeline` stored as JSON strings

### Async and Runtime

- Use Tokio for async operations
- Avoid blocking calls in async contexts
- If blocking is needed, wrap with `tokio::task::spawn_blocking`

### CLI

- Use `clap` derive
- Keep flags stable; document behavior in `SPEC.md` and `PLAYGROUND.md`
- Default output: pretty JSON; `--table` opt-in

### TUI

- Ratatui + Crossterm for rendering and input
- Keep state machines explicit and small
- Avoid hidden global state; pass state via structs

### Mongo

- Use official `mongodb` crate (async)
- Keep connection logic in `lazycompass-mongo`
- Avoid hard-coding database names; respect config defaults

### Files and Modules

- Prefer small, focused modules
- Keep public API minimal
- Put shared data models in `lazycompass-core`
- Put persistence and path resolution in `lazycompass-storage`

## Dependency Rules

- Prefer existing deps in `Cargo.toml` workspace deps
- New dependencies require clear justification
- Use LTS Rust and stable crate versions

## Git/Workspace Hygiene

- Solo workflow: commit and push directly to `main` (no PR required)
- CI validates pushes to `main`; local complete checks are mandatory before push
- Do not delete or rewrite existing user changes without approval
- Avoid destructive git commands
- Keep `.lazycompass/` contents in sync with sample data
- Proactively commit after finishing relevant work

## Release & repo maintenance

- Update `CHANGELOG.md` for user-visible changes; keep entries scoped to the release.
- Follow `RELEASE.md` for versioning/tagging and release steps before publishing.
- Run the checklist in `SECURITY.md` for dependency/config changes that affect security posture.
- Sync config defaults and examples per `CONFIGURATION.md` when changing config/saved spec schemas or paths.
- Validate query/aggregation format updates against `QUERY_FORMAT.md`, and note any schema changes in `CHANGELOG.md`.
