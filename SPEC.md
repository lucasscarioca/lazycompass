# LazyCompass Specification

Last updated: 2026-02-03

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
- Persist queries and aggregations in git-friendly TOML files
- Provide a fast CLI for running saved or inline queries/aggregations
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

### 8.3 Query and Aggregation Files (TOML)

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

## 11. UX Principles

- Keyboard-first, minimal mouse reliance
- Fast and responsive interactions
- Clear feedback for actions and errors
- Consistent keymap across views

## 12. Implementation Phases

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

- Keymap refinement
- Configurable themes
- Logging and telemetry (local)

## 13. Testing and Quality

- Unit tests for parsing and validation
- Integration tests for persistence resolution
- Basic CLI tests for command parsing
- CI later (future work)

## 14. References

- Yazi (Rust TUI inspiration): https://github.com/sxyazi/yazi
- Mongotui (Go TUI reference): https://github.com/kreulenk/mongotui
