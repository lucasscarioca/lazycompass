# LazyCompass

LazyCompass is a fast, vim-first MongoDB client for the terminal. It runs as a TUI by default, with CLI subcommands for running saved or inline queries/aggregations. Queries and aggregations can be persisted as git-committable TOML files so teams can share them per repo.

Status: **early stage, under active development**. Expect breaking changes.

> Disclaimer: LazyCompass is an independent open-source project and is not affiliated with or endorsed by MongoDB, Inc.

## Installation

From source (recommended for now, requires Rust toolchain):

```bash
./install.sh
```

Or via Cargo directly:

```bash
cargo install --path . -p lazycompass --locked
```

Upgrade (re-runs the installer):

```bash
lazycompass upgrade
```

## Usage

Start the TUI:

```bash
lazycompass
```

Run a saved query or aggregation:

```bash
lazycompass query active_users
lazycompass agg orders_by_user --table
```

Run an inline query or aggregation:

```bash
lazycompass query --db lazycompass --collection users --filter '{"active": true}'
lazycompass agg --db lazycompass --collection orders --pipeline '[{"$group": {"_id": "$userId", "total": {"$sum": "$total"}}}]'
```

## Configuration

LazyCompass resolves configuration in two places:

- Global: `~/.config/lazycompass/`
- Repo: `.lazycompass/` (overrides global)

Repo-scoped config (committable):

- `.lazycompass/config.toml`
- `.lazycompass/queries/*.toml`
- `.lazycompass/aggregations/*.toml`

Example query file:

```toml
name = "active_users"
connection = "local"
database = "lazycompass"
collection = "users"
filter = "{ \"active\": true }"
projection = "{ \"email\": 1, \"name\": 1, \"role\": 1 }"
sort = "{ \"createdAt\": -1 }"
limit = 50
notes = "Active users sorted by signup"
```

Example aggregation file:

```toml
name = "orders_by_user"
connection = "local"
database = "lazycompass"
collection = "orders"
pipeline = "[ { \"$group\": { \"_id\": \"$userId\", \"total\": { \"$sum\": \"$total\" }, \"count\": { \"$sum\": 1 } } }, { \"$sort\": { \"total\": -1 } } ]"
notes = "Total order spend per user"
```

## Local Playground

For a local MongoDB with seed data, see `PLAYGROUND.md`.

## Contributing

See `CONTRIBUTING.md`.

## License

MIT. See `LICENSE`.
