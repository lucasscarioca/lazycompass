# Query and aggregation formats

Saved queries and aggregations are read from the repo only:

- `.lazycompass/queries/*.toml`
- `.lazycompass/aggregations/*.toml`

Filenames are derived from the `name` field (slugged). Invalid files are skipped with warnings.

JSON fields are stored as strings for copy/paste parity with MongoDB syntax. MongoDB Extended JSON (relaxed or canonical) is supported.

## SavedQuery

Required:

- `name` (string)
- `database` (string)
- `collection` (string)

Optional:

- `connection` (string)
- `filter` (string, JSON)
- `projection` (string, JSON)
- `sort` (string, JSON)
- `limit` (integer)
- `notes` (string)

Example:

```toml
name = "active_users"
connection = "primary"
database = "app"
collection = "users"
filter = "{ \"active\": true }"
projection = "{ \"email\": 1, \"name\": 1 }"
sort = "{ \"createdAt\": -1 }"
limit = 100
notes = "Active users sorted by signup"
```

## SavedAggregation

Required:

- `name` (string)
- `database` (string)
- `collection` (string)
- `pipeline` (string, JSON array)

Optional:

- `connection` (string)
- `notes` (string)

Notes:

- Pipelines containing `$out` or `$merge` are blocked by default. Set `allow_pipeline_writes = true` and disable `read_only` to run them.

Example:

```toml
name = "orders_by_user"
connection = "primary"
database = "app"
collection = "orders"
pipeline = "[ { \"$group\": { \"_id\": \"$userId\", \"total\": { \"$sum\": \"$total\" } } }, { \"$sort\": { \"total\": -1 } } ]"
notes = "Total spend per user"
```

Extended JSON example:

```toml
filter = "{ \"_id\": { \"$oid\": \"64e1f2b4c2a3e02c9a0a9c10\" } }"
```
