# LazyCompass

LazyCompass is a fast, vim-first MongoDB client for the terminal. It runs as a TUI by default, with CLI subcommands for running saved or inline queries/aggregations. Queries and aggregations can be persisted as git-committable JSON files so teams can share them per repo.

> Disclaimer: LazyCompass is an independent open-source project and is not affiliated with or endorsed by MongoDB, Inc.

## Pre-1.0 stability

LazyCompass is pre-1.0. Breaking changes may happen in minor releases until 1.0; see [VERSIONING.md](./VERSIONING.md) and [CHANGELOG.md](./CHANGELOG.md) for details.

## Installation

Prebuilt binaries from GitHub releases:

- Supported release targets today: Linux x64 (glibc), macOS x64, macOS arm64.
- Other platforms can still use `cargo install --path . -p lazycompass --locked`.

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

Upgrade downloads `install.sh` from `raw.githubusercontent.com` for the official repo by default, or for the repo passed with `--repo`. It does not execute a local `install.sh` from the current directory.

Verification:

- The installer verifies release asset checksums when a `.sha256` file is present.
- If a `.sha256.sig` signature is present and `gpg` is installed, the installer verifies it too.
- See [SECURITY.md](./SECURITY.md) for vulnerability reporting and current release verification guidance.

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
# or for a write-enabled session
lazycompass --dangerously-enable-write
```

`--dangerously-enable-write` only enables MongoDB write operations. Local actions like saving queries/aggregations, editing config, exporting results, and clipboard copy remain available without it.

Env var naming:

- LazyCompass does not require a fixed URI var name. It resolves whatever you reference in config (`${VAR}`).
- `MONGO_URI` is a convention used in docs/examples; `MONGODB_URL` or `DATABASE_URL` also work if config uses that exact name.
- `.env` is auto-loaded from repo root for repo config and from `~/.config/lazycompass/.env` for global config.
- Real environment variables take precedence over `.env` values.
- Query and aggregation execution stops after 10,000 result documents; narrow the scope or add `--limit` for large result sets.
- Insecure Mongo connections are rejected by default; use `--allow-insecure` only for explicit local/trusted exceptions.

MongoDB write actions open your `$VISUAL` or `$EDITOR` for JSON editing (command + args only; no shell expansion).

Documents screen keys: `i` insert, `e` edit, `d` delete, `x` export results, `y` copy results, `Q` save query, `A` save aggregation, `r` run saved query, `a` run saved aggregation. Collections screen key: `I` list indexes. Connections screen key: `n` add connection.

Applied query/aggregation results can be exported from the TUI as JSON, CSV, or table text. Copy-to-clipboard uses native clipboard commands when available and falls back to OSC52. Result export/copy remains available without `--dangerously-enable-write`.

For local end-to-end validation against the bundled MongoDB playground, see [dev/qa/README.md](./dev/qa/README.md).

Run a saved query or aggregation:

```bash
lazycompass query app.users.active_users
lazycompass agg app.orders.orders_by_user --table
lazycompass query recent_orders --db app --collection orders
lazycompass query recent_orders --db app --collection orders -o results.json
lazycompass query recent_orders --db app --collection orders --csv -o results.csv
```

If the selected connection has `default_database` configured, you can omit `--db`:

```bash
lazycompass query --collection users --filter '{"active": true}'
lazycompass agg recent_orders --collection orders
```

Run an inline query or aggregation:

```bash
lazycompass query --db lazycompass --collection users --filter '{"active": true}'
lazycompass agg --db lazycompass --collection orders --pipeline '[{"$group": {"_id": "$userId", "total": {"$sum": "$total"}}}]'
```

Pipe or save CLI output:

```bash
lazycompass query --db app --collection users --filter '{"active": true}' | jq .
lazycompass agg recent_orders --collection orders --table > report.txt
lazycompass indexes --db app --collection users --table
lazycompass query recent_orders --db app --collection orders -o results.json
lazycompass query --collection users --filter '{"active": true}' --csv > users.csv
```

Manage config and data:

```bash
lazycompass init
lazycompass config edit
lazycompass config add-connection
lazycompass --dangerously-enable-write insert --db lazycompass --collection users --document '{"email": "a@example.com"}'
lazycompass --dangerously-enable-write update --db lazycompass --collection users --id '{"$oid":"64e1f2b4c2a3e02c9a0a9c10"}' --document '{"email": "a@example.com", "active": true}'
lazycompass --dangerously-enable-write insert --collection users --document '{"email": "a@example.com"}' # uses connection default_database
lazycompass --dangerously-enable-write update --collection users --id '{"$oid":"64e1f2b4c2a3e02c9a0a9c10"}' --document '{"email": "a@example.com", "active": true}' # uses connection default_database
```

## Configuration

See [CONFIGURATION.md](./CONFIGURATION.md) and [QUERY_FORMAT.md](./QUERY_FORMAT.md) for config and saved query formats.

## Docs

- [CONFIGURATION.md](./CONFIGURATION.md)
- [QUERY_FORMAT.md](./QUERY_FORMAT.md)
- [VERSIONING.md](./VERSIONING.md)
- [RELEASE.md](./RELEASE.md)
- [SECURITY.md](./SECURITY.md)
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)
- [SUPPORT.md](./SUPPORT.md)
- [CHANGELOG.md](./CHANGELOG.md)
- [dev/qa/README.md](./dev/qa/README.md)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

MIT. See `LICENSE`.
