# Configuration

LazyCompass loads config from two locations:

- Global: `~/.config/lazycompass/config.toml`
- Repo: `.lazycompass/config.toml`

Repo config overrides global. Connections are merged by name; a repo connection replaces a global connection with the same name. Other fields prefer the repo value when set, otherwise fall back to global.

Optional `.env` loading happens before config is parsed. LazyCompass looks for `.env` in the repo root and in `~/.config/lazycompass/` (in that order). Values from the real environment always win over `.env`.

Environment variables can be interpolated with `${VAR}` in:

- `connections[].uri`
- `logging.file`

Missing variables are an error.

Validation rules:

- Required fields must be non-empty.
- Connection names must be unique.
- Numeric settings must be greater than 0 when provided.

Defaults:

- `read_only`: true
- `allow_pipeline_writes`: false
- `allow_insecure`: false
- `timeouts.connect_ms`: 10000
- `timeouts.query_ms`: 30000
- `theme.name`: classic
- `logging.level`: info
- `logging.file`: `lazycompass.log` (resolved under the global config dir)
- `logging.max_size_mb`: 10
- `logging.max_backups`: 3

Write controls:

- `read_only` blocks database mutations and local writes (saved queries/aggregations and log file writes).
- `allow_pipeline_writes` allows `$out`/`$merge` stages when `read_only` is false.
- `allow_insecure` silences warnings for connections missing TLS or authentication.
- CLI overrides: `--write-enabled` disables `read_only`; `--allow-pipeline-writes` enables pipeline writes for the run; `--allow-insecure` silences TLS/auth warnings for the run.

File permissions (Unix):

- Config, saved query, saved aggregation, and temp editor files are written with `0600`.
- Config and saved query/aggregation directories are created with `0700`.
- LazyCompass warns when existing files or directories are more permissive.

Editor command:

- `$VISUAL`/`$EDITOR` are parsed as a command plus arguments (no shell expansion or pipes).

Config editing:

- `lazycompass config edit` opens the resolved config in your editor.
- `lazycompass config add-connection` adds a connection interactively.

## Config schema

### Root

- `read_only` (bool, optional)
- `allow_pipeline_writes` (bool, optional)
- `allow_insecure` (bool, optional)
- `connections` (array of ConnectionSpec, optional)
- `theme` (ThemeConfig, optional)
- `logging` (LoggingConfig, optional)
- `timeouts` (TimeoutConfig, optional)

### ConnectionSpec

- `name` (string, required)
- `uri` (string, required)
- `default_database` (string, optional)
  CLI fallback for `--db` in `query`, `agg`, `insert`, and `update`; also used for shared saved query/aggregation runs when `--db` is omitted.

### ThemeConfig

- `name` (string, optional)

Accepted values: `classic`, `default`, `ember`. `classic` and `default` are the same. Unknown values log a warning and fall back to `classic`.

### LoggingConfig

- `level` (string, optional)
- `file` (string, optional)
- `max_size_mb` (integer, optional)
- `max_backups` (integer, optional)

If `logging.file` is a relative path, it is resolved under `~/.config/lazycompass/`.

### TimeoutConfig

- `connect_ms` (integer, optional)
- `query_ms` (integer, optional)

## Example config.toml

```toml
read_only = true
allow_pipeline_writes = false
allow_insecure = false

[[connections]]
name = "primary"
uri = "${MONGO_URI}"
default_database = "app"

[theme]
name = "ember"

[logging]
level = "info"
file = "${LAZYCOMPASS_LOG}"
max_size_mb = 10
max_backups = 3

[timeouts]
connect_ms = 10000
query_ms = 30000
```

## Saved queries and aggregations

Saved queries and aggregations are loaded from repo-only files:

- `.lazycompass/queries/*.json`
- `.lazycompass/aggregations/*.json`

See `QUERY_FORMAT.md` for schemas and examples.
