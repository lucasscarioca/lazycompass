# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog and this project follows pre-1.0 Semantic Versioning.

## [Unreleased]

- TUI: add inline query/aggregation drafts so users can edit, run, retry, and save specs after validating results.
- TUI: add export-to-file and copy-to-clipboard for applied query/aggregation results, with JSON/CSV/table formats and single-document export from the document view.
- CLI: add `-o`/`--output` for `query` and `agg` to write rendered results to a file.
- CLI: add `--csv` output mode for `query` and `agg`, using the shared output renderer.
- Output: extract shared result rendering into a workspace crate and add CSV rendering for future CLI/TUI reuse.
- CLI: allow omitting `--db` for `query`, `agg`, `insert`, and `update` when the selected connection has `default_database`; shared saved query/aggregation runs now use the same fallback.
- Installer: stop surfacing raw `curl` 404 output when optional checksum/signature assets are unavailable during `lazycompass upgrade`.
- Storage: auto-normalize config/saved-spec permissions (`700` dirs, `600` files) on load to prevent recurring footer permission warnings.
- CLI: `lazycompass init` now ensures repo saved-spec directories exist (`.lazycompass/queries`, `.lazycompass/aggregations`).
- UX: improve query/aggregation failure messaging with explicit timeout guidance (`maxTimeMS`, `timeouts.query_ms`) and surfaced root causes in TUI/CLI.

## [0.6.1] - 2026-02-18

- CI: fix release workflow parsing by removing secret-based job-level conditional logic and handling optional signing with runtime guards.

## [0.6.0] - 2026-02-18

- Breaking: saved queries/aggregations now use JSON files (`.lazycompass/queries/*.json`, `.lazycompass/aggregations/*.json`) with dotted filename IDs (`<name>` shared, `<db>.<collection>.<name>` scoped); TOML saved specs removed.
- Security: harden defaults and docs with read-only/write guards, pipeline blocking, TLS/auth warnings, logging redaction, permission checks, safe editor usage, and installer verification guidance.
- Release: publish SHA256 checksum assets; publish checksum signatures when release signing secrets are configured.
- TUI: run saved queries and aggregations from the documents screen.
- TUI: add connection flow from the connections screen.
- TUI: allow canceling editor flows without mutating data.
- CLI: manage config via `config edit` and `config add-connection`.
- CLI: add `init` onboarding command to bootstrap config and first connection.
- CLI: add insert and update subcommands for documents.
- Config: load optional `.env` files for interpolation without overriding real env vars.
- Tests: print explicit skip reason when the playground integration test is not enabled.
- Docs: remove stale `SECURITY.md` reference from README.
- Dev workflow: remove Lefthook config; run checks manually before commit/push.
- Dev workflow: allow direct pushes to `main`; CI now runs on `main` pushes and manual dispatch.

## [0.5.0] - 2026-02-05

- Docs: publish configuration and query format references.
- Docs: add versioning, release process, and initial changelog.
- Docs: update README and contributing guidance for public release.
