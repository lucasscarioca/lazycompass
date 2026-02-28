# LazyCompass Reference

## Table of Contents

1. [Safety-First Operation](#1-safety-first-operation)
2. [Config + Path Model](#2-config--path-model)
3. [Command Surface (All Features)](#3-command-surface-all-features)
4. [Query/Agg Resolution Rules](#4-queryagg-resolution-rules)
5. [Saved Spec Formats](#5-saved-spec-formats)
6. [TUI Feature Map](#6-tui-feature-map)
7. [Common Errors and Fixes](#7-common-errors-and-fixes)

## 1) Safety-First Operation

- Treat every task as read-only unless user explicitly requests writes.
- Keep `read_only = true` by default.
- Do not use `--write-enabled` unless mutation is requested.
- Never run aggregation write stages (`$out`, `$merge`) unless user explicitly requests them and command includes `--allow-pipeline-writes`.
- In read-only mode, DB writes are blocked and local saved-spec/log writes are also blocked.
- `-o/--output` writes query/aggregation results to a file and is allowed for read-only query work.

Write controls:

- `--write-enabled`: disable read-only for current run.
- `--allow-pipeline-writes`: allow `$out`/`$merge` (still requires write-enabled).
- `--allow-insecure`: silence TLS/auth warnings for insecure connection URIs.

## 2) Config + Path Model

Config locations:

- Global: `~/.config/lazycompass/config.toml`
- Repo: `.lazycompass/config.toml`

Merge behavior:

- Repo config overrides global values when set.
- Connections merge by name; repo entry replaces same-name global entry.

Saved specs (repo-only):

- `.lazycompass/queries/*.json`
- `.lazycompass/aggregations/*.json`

Defaults:

- `read_only = true`
- `allow_pipeline_writes = false`
- `allow_insecure = false`
- `timeouts.connect_ms = 10000`
- `timeouts.query_ms = 30000`
- theme `classic`

Env interpolation:

- Supported in `connections[].uri` and `logging.file` with `${VAR}`.
- `.env` is loaded from repo root and `~/.config/lazycompass/`.
- Real environment values override `.env`.

## 3) Command Surface (All Features)

Open TUI:

```bash
lazycompass
```

Setup and config:

```bash
lazycompass init
lazycompass config edit
lazycompass config add-connection
```

Read operations:

```bash
lazycompass query <saved_id> [--db <db>] [--collection <collection>] [--connection <name>] [--table] [-o <path>]
lazycompass query --db <db> --collection <collection> [--filter '<json>'] [--projection '<json>'] [--sort '<json>'] [--limit <n>] [--connection <name>] [--table] [-o <path>]

lazycompass agg <saved_id> [--db <db>] [--collection <collection>] [--connection <name>] [--table] [-o <path>]
lazycompass agg --db <db> --collection <collection> --pipeline '<json array>' [--connection <name>] [--table] [-o <path>]
```

Write operations (explicit approval only):

```bash
lazycompass --write-enabled insert --db <db> --collection <collection> --document '<json>' [--connection <name>]
lazycompass --write-enabled insert --db <db> --collection <collection> --file <path-to-json> [--connection <name>]
lazycompass --write-enabled update --db <db> --collection <collection> --id '<json>' --document '<json>' [--connection <name>]
lazycompass --write-enabled update --db <db> --collection <collection> --id '<json>' --file <path-to-json> [--connection <name>]
```

Pipeline write stages (`$out`, `$merge`):

```bash
lazycompass --write-enabled --allow-pipeline-writes agg --db <db> --collection <collection> --pipeline '<json array>'
```

Upgrade:

```bash
lazycompass upgrade
lazycompass upgrade --version <tag>
lazycompass upgrade --from-source
lazycompass upgrade --repo <owner/name>
lazycompass upgrade --no-modify-path
```

Global flags usable with CLI or TUI launch:

- `--write-enabled`
- `--allow-pipeline-writes`
- `--allow-insecure`

Scope flags for setup/config:

- `lazycompass init --global`
- `lazycompass init --repo`
- `lazycompass config --global edit`
- `lazycompass config --repo add-connection`

## 4) Query/Agg Resolution Rules

Connection resolution:

- If exactly one connection exists, it is auto-selected.
- If multiple exist, pass `--connection`.

Database fallback:

- `query`, `agg`, `insert`, `update` can omit `--db` if selected connection has `default_database`.

Saved vs inline:

- Saved query/agg ID is filename stem in `.lazycompass/queries` or `.lazycompass/aggregations`.
- Saved IDs cannot be combined with inline payload flags.
- Scoped saved files (`<db>.<collection>.<name>.json`) resolve DB/collection from filename.
- Shared saved files (`<name>.json`) require collection context:
  CLI needs `--collection`, and DB comes from `--db` or connection `default_database`.
  TUI uses the current selected DB/collection.

Output:

- Default: pretty JSON
- Optional: `--table`
- File output: `-o/--output <path>`

## 5) Saved Spec Formats

Saved filename patterns:

- Shared: `<name>.json`
- Scoped: `<database>.<collection>.<name>.json`
- Two-segment filenames are invalid.

Saved query payload (`.lazycompass/queries/*.json`):

```json
{
  "filter": { "active": true },
  "projection": { "email": 1 },
  "sort": { "createdAt": -1 },
  "limit": 100
}
```

Allowed keys: `filter`, `projection`, `sort`, `limit`.

Saved aggregation payload (`.lazycompass/aggregations/*.json`):

```json
[
  { "$group": { "_id": "$userId", "total": { "$sum": "$total" } } },
  { "$sort": { "total": -1 } }
]
```

Payload must be a JSON array.

## 6) TUI Feature Map

Core navigation:

- `j/k` move
- `h` back
- `l` or Enter forward
- `gg` top, `G` bottom
- `?` help
- `q` quit

Documents screen actions:

- `i` insert document (write)
- `e` edit/replace document (write)
- `d` delete document (write)
- `Q` save query (local write)
- `A` save aggregation (local write)
- `r` run saved query
- `a` run saved aggregation
- `c` clear applied saved query/aggregation

Connections screen:

- `n` add connection

Use TUI write actions only with explicit user authorization.

## 7) Common Errors and Fixes

- `read-only mode: <action> is disabled`
Use read commands or explicitly run with `--write-enabled` if user requested mutation.

- `pipeline stage '$out' is blocked`
Use `--write-enabled --allow-pipeline-writes` only when user explicitly asks for pipeline writes.

- `multiple connections configured`
Pass `--connection <name>`.

- `--db is required ...`
Either pass `--db` or set `connections[].default_database` for the selected connection.

- `saved query/aggregation '<id>' not found`
Check `.lazycompass/queries/*.json` or `.lazycompass/aggregations/*.json` filename stems.
