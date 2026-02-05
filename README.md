# LazyCompass

LazyCompass is a fast, vim-first MongoDB client for the terminal. It runs as a TUI by default, with CLI subcommands for running saved or inline queries/aggregations. Queries and aggregations can be persisted as git-committable TOML files so teams can share them per repo.

> Disclaimer: LazyCompass is an independent open-source project and is not affiliated with or endorsed by MongoDB, Inc.

## Pre-1.0 stability

LazyCompass is pre-1.0. Breaking changes may happen in minor releases until 1.0; see `VERSIONING.md` and `CHANGELOG.md` for details.

## Installation

Prebuilt binaries from GitHub releases (no Rust required):

```bash
./install.sh
```

Install via curl (no Rust required):

```bash
curl -fsSL https://raw.githubusercontent.com/lucasscarioca/lazycompass/main/install.sh | bash
```

Build from source or install via Cargo (requires Rust toolchain):

```bash
cargo install --path . -p lazycompass --locked
```

Upgrade (re-runs the installer):

```bash
lazycompass upgrade
```

Override the installer URL (optional):

```bash
LAZYCOMPASS_INSTALL_URL=https://raw.githubusercontent.com/lucasscarioca/lazycompass/main/install.sh \
  lazycompass upgrade
```

## Usage

Start the TUI:

```bash
lazycompass
```

Write actions open your `$VISUAL` or `$EDITOR` for JSON/TOML editing.

Documents screen keys: `i` insert, `e` edit, `d` delete, `Q` save query, `A` save aggregation.

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

See `CONFIGURATION.md` and `QUERY_FORMAT.md` for config and saved query formats.

## Docs

- `CONFIGURATION.md`
- `QUERY_FORMAT.md`
- `SECURITY_REVIEW.md`
- `VERSIONING.md`
- `RELEASE.md`
- `CHANGELOG.md`

## Contributing

See `CONTRIBUTING.md`.

## License

MIT. See `LICENSE`.
