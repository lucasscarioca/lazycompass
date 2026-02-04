# LazyCompass Playground

This repo includes a self-contained MongoDB playground for local testing.

## Start the database

```bash
docker compose -f dev/mongodb/docker-compose.yml up -d
```

The seed script runs automatically on first container startup.

## Reset the data

```bash
docker compose -f dev/mongodb/docker-compose.yml down -v
docker compose -f dev/mongodb/docker-compose.yml up -d
```

## Example data

Database: `lazycompass`

Collections:
- `users`
- `orders`
- `events`

## Repo-scoped config

Repo config lives in `.lazycompass/` and is git-committable:

- `.lazycompass/config.toml`
- `.lazycompass/queries/*.toml`
- `.lazycompass/aggregations/*.toml`

## Try the CLI

```bash
cargo run -p lazycompass -- query active_users
cargo run -p lazycompass -- query --db lazycompass --collection users --filter '{"active": true}'
cargo run -p lazycompass -- agg orders_by_user --table
```

## Try the TUI

The TUI opens your `$VISUAL` or `$EDITOR` for JSON/TOML editing.

```bash
cargo run -p lazycompass
```

Keys (Documents screen): `i` insert, `e` edit, `d` delete, `Q` save query, `A` save aggregation.
