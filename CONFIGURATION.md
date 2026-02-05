# Configuration

LazyCompass loads config from two locations:

- Global: `~/.config/lazycompass/config.toml`
- Repo: `.lazycompass/config.toml`

Repo config overrides global. Connections are merged by name; a repo connection replaces a global connection with the same name. Other fields prefer the repo value when set, otherwise fall back to global.

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
- `timeouts.connect_ms`: 10000
- `timeouts.query_ms`: 30000
- `theme.name`: classic
- `logging.level`: info
- `logging.file`: `lazycompass.log` (resolved under the global config dir)
- `logging.max_size_mb`: 10
- `logging.max_backups`: 3

## Config schema

### Root

- `read_only` (bool, optional)
- `connections` (array of ConnectionSpec, optional)
- `theme` (ThemeConfig, optional)
- `logging` (LoggingConfig, optional)
- `timeouts` (TimeoutConfig, optional)

### ConnectionSpec

- `name` (string, required)
- `uri` (string, required)
- `default_database` (string, optional)

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

- `.lazycompass/queries/*.toml`
- `.lazycompass/aggregations/*.toml`

See `QUERY_FORMAT.md` for schemas and examples.
