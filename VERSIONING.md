# Versioning

LazyCompass is currently in the last pre-1.0 stabilization stretch for the CLI and TUI.

## Current status (v0.x)

- Breaking changes may occur in minor releases until 1.0.
- Patch releases are for fixes and small improvements.
- CLI flags, config schema, and saved query formats are treated as public API, but may change in minor releases before 1.0.

## Planned 1.x policy

- `1.x` releases follow Semantic Versioning (SemVer).
- Breaking CLI, config, or saved-spec changes require a major version bump.
- Linux and macOS are the stable `1.x` targets.
- Windows remains beta until its release/install/upgrade path reaches parity with the stable targets.

## Schema notes

Config and query schemas do not include an explicit schema version. Any config or schema changes are documented in [CHANGELOG.md](./CHANGELOG.md) with migration guidance when needed.
