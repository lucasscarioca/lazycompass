# Versioning

LazyCompass follows pre-1.0 Semantic Versioning (SemVer) for the CLI and TUI.

## Compatibility policy (v0.x)

- Breaking changes may occur in minor releases until 1.0.
- Patch releases are for fixes and small improvements.
- CLI flags, config schema, and saved query formats are treated as public API, but may change in minor releases before 1.0.

## Schema notes

Config and query schemas do not include an explicit schema version. Any config or schema changes are documented in `CHANGELOG.md` with migration guidance when needed.
