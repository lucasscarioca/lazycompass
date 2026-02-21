---
name: lazycompass-operator
description: "Operate LazyCompass (MongoDB TUI + CLI client) safely across all features: init, config management, query/aggregation execution, saved specs, TUI navigation, document writes, and upgrade flows. Use when an AI agent must run `lazycompass` commands, edit `.lazycompass` config/spec files, or troubleshoot read-only and pipeline-write guard behavior."
---

# LazyCompass Operator

Use this skill to run LazyCompass as an AI agent without risking accidental database mutations.

## Safety Contract (Mandatory)

1. Default to read-only behavior for every task.
2. Never enable `--write-enabled` unless user explicitly asks for a DB mutation.
3. Never run `insert`, `update`, TUI `i/e/d`, or aggregation pipelines with `$out`/`$merge` unless user explicitly authorizes writes.
4. Prefer CLI commands for agentic execution; use TUI only when user explicitly asks for interactive flow.
5. Keep write scope minimal: single connection, single database, single collection, explicit payload.
6. For risky write requests, show the exact command before executing.

Read `references/lazycompass-reference.md` before execution when you need exact command forms, file schemas, or edge-case behavior.

## Workflow

1. Resolve scope and config source.
Check repo-local `.lazycompass/config.toml` first; merge/fallback to `~/.config/lazycompass/config.toml`.

2. Classify intent.
Use read-only path for browsing/querying/troubleshooting. Use write path only for explicit insert/update/pipeline-write requests.

3. Choose execution mode.
Use CLI for deterministic runs and captured output. Use TUI only if user asks for interactive navigation.

4. Execute least-privilege command.
Prefer `query`/`agg` first. Pass `--connection` when multiple connections exist.

5. Report outcome and guardrails.
Include what was run, target connection/db/collection, and whether read-only protections remained enabled.

## Read-Only Defaults To Preserve

- Config default is `read_only = true`.
- `query` and non-writing `agg` work in read-only mode.
- `insert`/`update` are blocked in read-only mode.
- `$out`/`$merge` are blocked unless both write mode and pipeline-write mode are enabled.
- In read-only mode, log file writes and saved query/aggregation writes are blocked.

## Write Escalation Procedure

1. Confirm explicit user intent to mutate DB.
2. Enable write mode only for that command via `--write-enabled`.
3. For `$out`/`$merge`, also add `--allow-pipeline-writes`.
4. Avoid persisting permissive config unless user explicitly asks.
5. After write task, return to read-only commands.

## Feature Coverage Map

Use `references/lazycompass-reference.md` for complete details on:
- setup (`init`, `config edit`, `config add-connection`)
- querying and aggregations (saved + inline)
- output modes (`json` default, `--table` optional)
- TUI actions and keybindings
- document writes (`insert`, `update`, TUI edit/delete/insert)
- saved query/aggregation JSON formats and filename scope
- upgrade command behavior
- common failure patterns and fixes
