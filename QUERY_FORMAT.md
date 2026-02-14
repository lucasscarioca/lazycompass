# Query and aggregation formats

Saved queries and aggregations are read from the repo only:

- `.lazycompass/queries/*.json`
- `.lazycompass/aggregations/*.json`

Only files with `.json` extension are loaded. Invalid files are skipped with warnings.

## Filename IDs and scope

Saved spec ID is the filename stem (filename without `.json`), matched exactly by CLI/TUI.

Filename patterns:

- Shared: `<name>.json`
- Scoped: `<database>.<collection>.<name>.json`

Rules:

- 1 segment: shared.
- 3+ segments: scoped (`database = first`, `name = last`, `collection = middle segments joined by "."`).
- 2 segments are invalid.
- Empty segments (like `a..b`) are invalid.

Examples:

- `.lazycompass/queries/recent_orders.json` (shared)
- `.lazycompass/queries/app.users.active_users.json` (scoped)
- `.lazycompass/queries/app.foo.bar.orders.by_user.json` (scoped, collection is `foo.bar.orders`)

## SavedQuery JSON payload

Payload is a JSON object, metadata-free.

Allowed keys:

- `filter` (JSON value)
- `projection` (JSON value)
- `sort` (JSON value)
- `limit` (non-negative integer)

All keys are optional. `{}` is valid.

Example:

```json
{
  "filter": { "active": true },
  "projection": { "email": 1, "name": 1 },
  "sort": { "createdAt": -1 },
  "limit": 100
}
```

## SavedAggregation JSON payload

Payload is a JSON array (Mongo pipeline), metadata-free.

Example:

```json
[
  { "$group": { "_id": "$userId", "total": { "$sum": "$total" } } },
  { "$sort": { "total": -1 } }
]
```

## Runtime target resolution

- Scoped files: database/collection come from filename.
- Shared files:
  - CLI: require `--db` and `--collection` when running saved specs.
  - TUI: use current selected database/collection.
- Connection is runtime-selected (`--connection` in CLI or selected connection in TUI).

## Pipeline write safety

Pipelines containing `$out` or `$merge` are blocked by default. Set `allow_pipeline_writes = true` and disable `read_only` to run them.
