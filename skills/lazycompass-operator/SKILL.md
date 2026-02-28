---
name: lazycompass-operator
description: "Safely operate the LazyCompass MongoDB CLI with a read-only-first workflow. Use when an AI agent needs to inspect `.lazycompass` config, resolve connection/database/collection context, run saved or inline `lazycompass query` or `lazycompass agg` commands, capture results with `-o/--output`, or give brief user guidance for the TUI. Use for `insert`, `update`, or aggregation pipeline writes only when the user explicitly authorizes a database mutation."
---

# LazyCompass Operator

Operate LazyCompass with a CLI-first, read-only-first workflow. Prefer deterministic commands and captured output over interactive TUI usage.

Read `references/lazycompass-reference.md` only when you need exact command syntax, saved-spec formats, or specific error text.

## Safety Rules

1. Treat every task as read-only unless the user explicitly asks to mutate MongoDB.
2. Prefer `lazycompass query` and `lazycompass agg` for agent work.
3. Keep scope explicit with `--connection`, `--db`, and `--collection` whenever resolution is ambiguous.
4. Use `-o/--output <path>` when the agent needs a durable artifact; this is a local file write, not a database mutation.
5. Do not use `--write-enabled`, `insert`, `update`, or `$out`/`$merge` unless the user explicitly authorizes a DB write.
6. Show the exact write command before executing risky mutations.

## Fast Workflow

1. Resolve scope and config source.
Check repo-local `.lazycompass/config.toml` first, then global `~/.config/lazycompass/config.toml`.

2. Classify intent.
Choose read-only flow for inspection, querying, troubleshooting, and result export. Choose write flow only for explicit insert, update, or pipeline-write requests.

3. Choose execution mode.
Use CLI by default. Use TUI only if the user asks for an interactive walkthrough.

4. Execute least-privilege command.
Prefer saved specs when they already exist. Otherwise run an inline `query` or `agg`. Pass `--connection` when multiple connections exist.

5. Report outcome and guardrails.
Report the exact command, resolved connection/database/collection, output mode, and whether read-only protections remained enabled.

## CLI Patterns

Read-only query paths:

```bash
lazycompass query <saved_id> [--db <db>] [--collection <collection>] [--connection <name>] [--table] [-o <path>]
lazycompass query --db <db> --collection <collection> [--filter '<json>'] [--projection '<json>'] [--sort '<json>'] [--limit <n>] [--connection <name>] [--table] [-o <path>]

lazycompass agg <saved_id> [--db <db>] [--collection <collection>] [--connection <name>] [--table] [-o <path>]
lazycompass agg --db <db> --collection <collection> --pipeline '<json array>' [--connection <name>] [--table] [-o <path>]
```

Output behavior:

- Default output is pretty JSON to stdout.
- `--table` prints a scalar-field table.
- `-o/--output` writes the rendered result to a file instead of stdout.
- `-o` works with both JSON and table output.

Resolution rules:

- If one connection exists, LazyCompass auto-selects it. If multiple exist, pass `--connection`.
- `query`, `agg`, `insert`, and `update` can omit `--db` when the selected connection has `default_database`.
- Saved IDs cannot be combined with inline payload flags like `--filter` or `--pipeline`.
- Scoped saved specs (`<db>.<collection>.<name>.json`) provide DB and collection from the filename.
- Shared saved specs (`<name>.json`) still need collection context, and DB must come from `--db` or the connection default.

## Write Escalation Procedure

1. Confirm explicit user intent to mutate MongoDB.
2. Use `lazycompass --write-enabled insert ...` or `lazycompass --write-enabled update ...` only for that single command.
3. For `$out` or `$merge`, also add `--allow-pipeline-writes`.
4. Avoid changing config to make writes persistent unless the user explicitly asks.
5. Return to read-only commands immediately after the write task.

## TUI Fallback

Only guide TUI usage when the user explicitly wants it:

- `lazycompass` opens the TUI.
- Basic navigation: `j/k`, `h`, `l` or Enter, `?`, `q`.
- Query helpers: `r` run saved query, `a` run saved aggregation, `c` clear applied saved spec.
- Local writes: `Q` save query, `A` save aggregation.
- DB writes: `i` insert, `e` edit, `d` delete.

Treat `Q`, `A`, `i`, `e`, and `d` as write actions that need explicit user intent.
