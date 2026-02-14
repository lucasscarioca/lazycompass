# LazyCompass

LazyCompass is a fast, vim-first MongoDB client for the terminal. It runs as a TUI by default, with CLI subcommands for running saved or inline queries/aggregations. Queries and aggregations can be persisted as git-committable JSON files so teams can share them per repo.

> Disclaimer: LazyCompass is an independent open-source project and is not affiliated with or endorsed by MongoDB, Inc.

## Pre-1.0 stability

LazyCompass is pre-1.0. Breaking changes may happen in minor releases until 1.0; see `VERSIONING.md` and `CHANGELOG.md` for details.

## Installation

Prebuilt binaries from GitHub releases:

```bash
./install.sh
```

Install via curl:

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

Verification:

- The installer verifies release asset checksums when a `.sha256` file is present.
- If a `.sha256.sig` signature is present and `gpg` is installed, the installer verifies it too.

Manual verification example (Linux x64):

```bash
curl -LO https://github.com/lucasscarioca/lazycompass/releases/latest/download/lazycompass-linux-x64.tar.gz
curl -LO https://github.com/lucasscarioca/lazycompass/releases/latest/download/lazycompass-linux-x64.tar.gz.sha256
curl -LO https://github.com/lucasscarioca/lazycompass/releases/latest/download/lazycompass-linux-x64.tar.gz.sha256.sig

gpg --verify lazycompass-linux-x64.tar.gz.sha256.sig lazycompass-linux-x64.tar.gz.sha256
sha256sum -c lazycompass-linux-x64.tar.gz.sha256 2>/dev/null || shasum -a 256 -c lazycompass-linux-x64.tar.gz.sha256
```

## Usage

Quick start (existing repo):

1. `cd` into the repo (or any subdirectory inside it).
2. Run the setup wizard (creates/updates `.lazycompass/config.toml` and adds a connection):

```bash
lazycompass init
```

3. If your connection URI in config uses env interpolation (example below), set that variable:

```toml
[[connections]]
name = "primary"
uri = "${MONGO_URI}"
default_database = "app"
```

```bash
export MONGO_URI='mongodb://localhost:27017/app'
```

or put it in repo root `.env`:

```dotenv
MONGO_URI=mongodb://localhost:27017/app
```

4. Start LazyCompass:

```bash
lazycompass
```

Env var naming:

- LazyCompass does not require a fixed URI var name. It resolves whatever you reference in config (`${VAR}`).
- `MONGO_URI` is a convention used in docs/examples; `MONGODB_URL` or `DATABASE_URL` also work if config uses that exact name.
- `.env` is auto-loaded from repo root for repo config and from `~/.config/lazycompass/.env` for global config.
- Real environment variables take precedence over `.env` values.

Write actions open your `$VISUAL` or `$EDITOR` for JSON editing (command + args only; no shell expansion).

Documents screen keys: `i` insert, `e` edit, `d` delete, `Q` save query, `A` save aggregation, `r` run saved query, `a` run saved aggregation. Connections screen key: `n` add connection.

Run a saved query or aggregation:

```bash
lazycompass query app.users.active_users
lazycompass agg app.orders.orders_by_user --table
lazycompass query recent_orders --db app --collection orders
```

Run an inline query or aggregation:

```bash
lazycompass query --db lazycompass --collection users --filter '{"active": true}'
lazycompass agg --db lazycompass --collection orders --pipeline '[{"$group": {"_id": "$userId", "total": {"$sum": "$total"}}}]'
```

Manage config and data:

```bash
lazycompass init
lazycompass config edit
lazycompass config add-connection
lazycompass insert --db lazycompass --collection users --document '{"email": "a@example.com"}'
lazycompass update --db lazycompass --collection users --id '{"$oid":"64e1f2b4c2a3e02c9a0a9c10"}' --document '{"email": "a@example.com", "active": true}'
```

## Configuration

See `CONFIGURATION.md` and `QUERY_FORMAT.md` for config and saved query formats.

## Docs

- `CONFIGURATION.md`
- `QUERY_FORMAT.md`
- `VERSIONING.md`
- `RELEASE.md`
- `CHANGELOG.md`

## Contributing

See `CONTRIBUTING.md`.

## License

MIT. See `LICENSE`.
