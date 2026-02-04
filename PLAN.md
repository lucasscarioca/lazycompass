# LazyCompass MVP Plan

This plan captures the next four phases for delivering a working MVP, aligned with `SPEC.md`.

## Phase A — Config + Persistence (Unblocks CLI + TUI)

Goal: Make saved queries/aggregations and connection config loadable and reliable.

Status: complete (2026-02-04)

Scope:
- Implement loaders in `lazycompass-storage`:
  - Global config: `~/.config/lazycompass/config.toml`
  - Repo config: `.lazycompass/config.toml`
  - Repo overrides global using merge-by-name (repo wins on collisions)
- Load saved queries/aggregations from:
  - `.lazycompass/queries/*.toml`
  - `.lazycompass/aggregations/*.toml`
- Validate required fields with clear, actionable errors.
- Add helper validation functions in `lazycompass-core` for `SavedQuery` and `SavedAggregation`.
- Ensure errors include path context for easier debugging.

Deliverables:
- Storage API for resolving config + saved specs.
- Tests for config merge and file parsing (if feasible now).

## Phase B — Mongo Execution (CLI works against playground)

Goal: Make `lazycompass query/agg` actually execute against MongoDB.

Status: complete (2026-02-04)

Scope:
- Implement connection resolution by name in `lazycompass-mongo`.
- Query execution:
  - Parse filter/projection/sort JSON strings.
  - Run `find` with limit.
- Aggregation execution:
  - Parse pipeline JSON string and run `aggregate`.
- Wire CLI commands to:
  - Load saved specs from storage
  - Execute inline specs
  - Output pretty JSON (default)
  - Output basic table for top-level scalar fields (`--table`)

Deliverables:
- CLI end-to-end against the playground.
- Minimal table formatter for MVP.

## Phase C — TUI MVP (Read-only)

Goal: Provide a working, read-only TUI flow.

Status: complete (2026-02-04)

Scope:
- TUI application shell using ratatui + crossterm.
- State machine:
  - Connection selection
  - Database list
  - Collection list
  - Document list with pagination
  - Document view
- Vim-like navigation keys (j/k/h/l, gg/G, q).
- Read-only data access via shared storage + mongo layers.

Deliverables:
- Basic navigation and document viewing.
- No mutations yet (edit/insert/delete later).

## Phase D — Write Actions (Post-MVP)

Goal: Enable edits, inserts, and deletion with safe UX.

Scope:
- Document insert/edit/delete.
- Optional $EDITOR integration for editing JSON.
- Save queries/aggregations from TUI.
- Improve keymap and command palette interactions.

Deliverables:
- Full CRUD in TUI.
- Persistence of saved specs from UI.

## Decisions Locked In

- Table output is basic (top-level scalar fields only) for MVP.
- Config merge uses merge-by-name; repo overrides global.
- TUI MVP is read-only; write actions in Phase D.
