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

Start the TUI:

```bash
lazycompass
```

Write actions open your `$VISUAL` or `$EDITOR` for JSON/TOML editing (command + args only; no shell expansion).

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
- `SECURITY.md`
- `VERSIONING.md`
- `RELEASE.md`
- `CHANGELOG.md`

## Contributing

See `CONTRIBUTING.md`.

## License

MIT. See `LICENSE`.
