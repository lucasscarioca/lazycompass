# LazyCompass Specification

Last updated: 2026-02-04

## 1. Overview

LazyCompass is an open-source, non-commercial MongoDB client that runs in a terminal user interface (TUI). It provides a fast, vim-like navigation model for exploring and editing MongoDB resources, inspired by tools like lazygit and yazi. LazyCompass also exposes a CLI for fast, script-friendly queries and aggregations. Queries and aggregations can be persisted globally or per-repository in git-committable files to support team collaboration.

This document is the source of truth for the project's idea, vision, requirements, and implementation phases.

## 2. Vision

Build a fast, ergonomic, and collaborative MongoDB TUI that:

- Feels as responsive and fluid as a file manager
- Makes complex database operations approachable
- Enables teams to share queries and aggregation pipelines as code
- Encourages reproducible, reviewable data operations through git

## 3. Goals

- Provide a vim-like navigation and interaction model
- Offer an efficient UI for browsing and editing databases, collections, and documents
- Support read/write operations, aggregations, and schema exploration
- Support secure production connections (TLS/SRV/auth) and safe-by-default operations
- Persist queries and aggregations in git-friendly TOML files
- Provide a fast CLI for running saved or inline queries/aggregations
- Provide reliable, observable behavior (structured logs, clear errors, exit codes)
- Keep config formats stable with backward-compatible changes and migration notes
- Maintain a clean, modular Rust workspace aligned with the Yazi stack

## 4. Non-Goals (Initial)

- Full MongoDB admin tooling (cluster management, backups, users/roles)
- Atlas-specific features (may be added later)
- GUI or web interfaces
- Remote server agent (client-only for now)

## 5. Target Users

- Developers and data engineers working with MongoDB
- Teams that want to standardize queries and aggregations in git
- Terminal power users who prefer keyboard-first workflows

## 6. Tech Stack (Aligned with Yazi)

- Language: Rust (edition 2024, LTS toolchain)
- Async runtime: Tokio
- TUI: Ratatui + Crossterm (event-stream)
- CLI: Clap
- Serialization: Serde + TOML
- Error handling: Anyhow + ThisError
- Logging/Tracing: Tracing
- Paths and config: Dirs
- MongoDB driver: mongodb (async)

## 7. Architecture

### 7.1 Workspace Layout

Rust workspace with focused crates:

- lazycompass: unified binary (TUI by default, CLI via subcommands)
- lazycompass-tui: TUI library
- lazycompass-core: shared domain models and validation
- lazycompass-storage: config, persistence, and path resolution
- lazycompass-mongo: MongoDB connection and execution layer

### 7.2 Data Flow

CLI/TUI -> core models -> storage (read config/specs) -> mongo (execute) -> output (json/table)

### 7.3 Configuration Resolution

- Global: ~/.config/lazycompass
- Repo: <repo>/.lazycompass
- Repo overrides global, with fallback to global
- Repo config must be safe to commit; keep secrets in global config or env vars

## 8. Persistence and File Layout

### 8.1 Global

~/.config/lazycompass/
  config.toml
  queries/
  aggregations/

### 8.2 Repo

<repo>/.lazycompass/
  config.toml
  queries/
  aggregations/

### 8.3 Config File (TOML)

Example:

```
read_only = true

[timeouts]
connect_ms = 10000
query_ms = 30000

[[connections]]
name = "local"
uri = "mongodb://localhost:27017"
default_database = "lazycompass"

[theme]
name = "classic"

[logging]
level = "info"
file = "lazycompass.log"
max_size_mb = 10
max_backups = 3
```

Theme notes:

- `theme.name` is optional; defaults to `classic`.
- Available built-in themes: `classic`, `ember`.

Connection notes:

- MongoDB connection URIs must support TLS/SRV/auth options used in production.
- Prefer keeping credentials in global config or env vars, not repo config.
- Support env var interpolation in config values (example: `${MONGO_URI}`).
- Redact connection URIs in logs and errors by masking credentials as `***`.

Logging notes:

- `logging.level` defaults to `info`.
- `logging.file` defaults to `~/.config/lazycompass/lazycompass.log`.
- Relative `logging.file` paths are resolved against the global config directory.
- `logging.max_size_mb` defaults to 10.
- `logging.max_backups` defaults to 3.

Read-only notes:

- `read_only` defaults to true; set to false to enable writes.

Timeout notes:

- `timeouts.connect_ms` defaults to 10_000 (10s).
- `timeouts.query_ms` defaults to 30_000 (30s).

### 8.4 Query and Aggregation Files (TOML)

Queries and aggregations are stored as one-file-per-definition for easy review and git diffs.

Query file example:

```
name = "active_users"
connection = "prod"
database = "app"
collection = "users"
filter = "{ \"active\": true }"
projection = "{ \"email\": 1, \"name\": 1 }"
sort = "{ \"createdAt\": -1 }"
limit = 100
notes = "Active users for weekly report"
```

Aggregation file example:

```
name = "daily_signups"
connection = "prod"
database = "app"
collection = "users"
pipeline = "[ { \"$match\": { \"active\": true } }, { \"$group\": { \"_id\": \"$day\", \"count\": { \"$sum\": 1 } } } ]"
notes = "Signup counts by day"
```

The filter/projection/sort/pipeline values are JSON strings for compatibility with MongoDB syntax and easy copy/paste.
These strings accept MongoDB Extended JSON (relaxed or canonical) so BSON types like ObjectId and dates can be represented.

Example (Extended JSON ObjectId):

```
filter = "{ \"_id\": { \"$oid\": \"64e1f2b4c2a3e02c9a0a9c10\" } }"
```

## 9. CLI Requirements

- CLI is available via `lazycompass <command>` (default runs TUI)
- Run a saved query or aggregation by name
- Run inline query/aggregation without saving
- Default output is pretty JSON
- Optional `--table` flag for table output
- Support a read-only mode that blocks write operations

Example commands:

```
lazycompass query active_users
lazycompass agg daily_signups
lazycompass query --db app --collection users --filter '{"active": true}'
lazycompass agg --db app --collection users --pipeline '[{"$match": {"active": true}}]'
```

## 10. TUI Requirements (MVP)

- Connection selector
- Databases list
- Collections list
- Documents list with pagination
- Document view with basic edit and delete
- Query/aggregation editor panel
- Vim-like navigation and key bindings
- Read-only mode that disables writes and highlights safety status

## 11. UX Principles

- Keyboard-first, minimal mouse reliance
- Fast and responsive interactions
- Clear feedback for actions and errors
- Consistent keymap across views
- Provide inline key hints and a help overlay (`?` to open, `Esc` to close)
- Safety-first UX for destructive actions (explicit confirmation + context)

## 12. Production Readiness Requirements

### 12.1 Security and Secrets

- Support TLS/SRV/auth via MongoDB URIs
- Allow env var interpolation for config values
- Never log secrets (redact credentials and tokens)
- Prefer global config for credentials; repo config must remain secret-free

### 12.2 Safety and Guardrails

- Default to read-only unless explicitly enabled
- Destructive actions require explicit confirmation with clear context
- Provide safe previews for write operations where possible

### 12.3 Reliability and Performance

- Configurable timeouts for connect and query operations
- Clear handling for transient errors and network disconnects
- Non-blocking UI interactions with visible loading/error states
- No automatic retries yet; re-run safe read-only operations on transient failures
- Never retry writes automatically unless the operation is explicitly idempotent

### 12.4 Observability and Operations

- Structured logs with component and command fields
- Consistent CLI exit codes for automation
- Log file rotation or size limits

Exit codes:

- 0: success
- 1: user error (invalid args, missing inputs)
- 2: config error (TOML or config file issues)
- 3: network error (connection/timeout)

### 12.5 Release and Compatibility

- Semantic versioning for user-facing changes
- Config schema versioning with migration notes

## 13. Implementation Phases

### Phase 1: Workspace and Core

- Create workspace and crates
- Add shared dependencies
- Define core models and validation

### Phase 2: Persistence

- Implement config loading with repo/global resolution
- Parse and validate query/aggregation TOML
- Add sample config and spec examples

### Phase 3: CLI

- Implement query/aggregation execution pipeline
- Add output formatting (pretty JSON default, table option)

### Phase 4: Mongo Integration

- Connection management
- Query and aggregation execution
- Error handling and retry guidance

### Phase 5: TUI MVP

- Screen flow and state management
- Document browsing and viewing
- Minimal editor integration

### Phase 6: Polish

Status: complete (2026-02-04)

- Keymap refinement
- Configurable themes
- Logging and telemetry (local)

### Phase 7: Production Readiness

Status: complete (2026-02-04)

- Secure connection support (TLS/SRV/auth) + secret handling
- Read-only mode and safer write workflows
- Operational observability (structured logs, exit codes, rotation)
- Reliability improvements (timeouts, retry guidance)
- CI gating (fmt/clippy/test) and release checklist

## 14. Testing and Quality

- Unit tests for parsing and validation
- Integration tests for persistence resolution
- Basic CLI tests for command parsing
- Added tests for keymap validation, theme selection, and log path resolution
- CI must run fmt, clippy, and full test suite
- Integration tests against the Docker MongoDB playground

## 15. References

- Yazi (Rust TUI inspiration): https://github.com/sxyazi/yazi
- Mongotui (Go TUI reference): https://github.com/kreulenk/mongotui
