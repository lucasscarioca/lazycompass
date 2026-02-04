# LazyCompass Phase 7 Plan

This plan focuses on Phase 7 (Production Readiness) from `SPEC.md`.

Status: in progress (2026-02-04)

## Phase 7 — Production Readiness

Goal: Make LazyCompass safe and reliable for production usage with secure connections,
explicit safety guardrails, and operational readiness.

### Scope

1) Secure connections and secrets handling
2) Safety-first workflows and read-only mode
3) Reliability and operational observability
4) CI gating + integration testing against the MongoDB playground

## Workstreams and Tasks

## Progress Update (2026-02-04)

- Added URI redaction for error output in CLI/TUI
- Added retry guidance for network errors in CLI/TUI
- CI now starts MongoDB playground and runs integration tests

### 7.1 Security and Secrets

Status: complete

Scope:
- Support MongoDB URI features used in production (TLS/SRV/auth options)
- Allow env var interpolation for config values
- Ensure secrets are never logged

Tasks:
- [x] Add env var interpolation to config loading
  - Resolve `${VAR}` placeholders in config values (connections.uri, logging.file)
  - Define behavior when env var is missing (error + clear message)
- [x] Introduce explicit redaction for connection URIs
  - Mask password/credentials in logs and error messages
  - Keep redaction consistent for TUI + CLI paths
- [x] Document secret handling rules in `SPEC.md`

Deliverables:
- Config can safely reference `${MONGO_URI}`
- Logs never include passwords or tokens
- Updated spec and examples

### 7.2 Safety and Guardrails

Status: complete

Scope:
- Default to read-only unless explicitly enabled
- Clear safety cues for destructive operations

Tasks:
- [x] Add a read-only toggle in config and CLI flag
  - CLI: `--read-only` (or `--write-enabled` with default off)
  - TUI: visible banner/status when read-only is active
- [x] Block write operations in read-only mode
  - Insert/edit/delete in TUI
  - CLI write paths (future: inline writes)
- [x] Improve destructive confirmations
  - Show context (connection, db, collection, _id)
  - Require explicit confirmation keyword for deletes (ex: type `delete`)

Deliverables:
- Read-only mode enforced in all write paths
- Stronger confirmation UX

### 7.3 Reliability and Performance

Status: complete

Scope:
- Add timeouts for network operations
- Improve handling of transient errors

Tasks:
- [x] Add connect and query timeout configuration
  - Expose defaults in config
  - Ensure timeouts are applied in mongo executor
- [x] Provide retry guidance or automatic retry for safe operations
  - Document behavior and limits
- [x] Improve error surfaces
  - CLI: clear error messages + exit codes
  - TUI: visible status and non-blocking UI
- [x] Make TUI data loads non-blocking
  - Move Mongo calls to background tasks
  - Render loading and error states without freezing UI

Deliverables:
- Timeouts enforced for connect/query
- Clear and consistent error handling

### 7.4 Observability and Operations

Status: complete

Scope:
- Structured logs with consistent fields
- Log rotation or size limiting
- Consistent exit codes

Tasks:
- [x] Add structured logging fields
  - command, component, connection name (redacted), database, collection
- [x] Implement log file rotation or max-size cap
  - Define retention policy in config
- [x] Define exit codes for CLI
  - Success, user error, config error, network error

Deliverables:
- Structured logs suitable for ops
- Controlled log size
- Documented exit codes

### 7.5 CI and Testing

Status: complete

Scope:
- CI gating for fmt/clippy/tests
- Integration tests using the Docker MongoDB playground

Tasks:
- [x] Add CI workflow
  - Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`
- [x] Add integration tests
  - Use docker-compose playground from `PLAYGROUND.md`
  - Validate connection + query + aggregation flows
- [x] Add tests for read-only mode and env interpolation

Deliverables:
- CI runs on PRs and main
- Reliable integration coverage

## Acceptance Criteria

- Secure connections: TLS/SRV/auth URIs work; secrets never appear in logs
- Read-only mode blocks all writes and is clearly visible in UX
- Timeouts and error handling provide predictable behavior
- Logs are structured and bounded in size
- CI gating is active; integration tests pass

## Risks and Mitigations

- Risk: Env var interpolation hides errors
  - Mitigation: fail fast with explicit error message
- Risk: Read-only mode blocks legitimate workflows
  - Mitigation: explicit opt-in for write mode; clear status
- Risk: Log rotation complexity
  - Mitigation: start with max-size cap and simple rollover

## Dependencies

- `lazycompass-core` for config schema changes
- `lazycompass-storage` for config resolution and env interpolation
- `lazycompass-mongo` for timeout handling
- `lazycompass-cli` for exit codes and read-only flag
- `lazycompass-tui` for read-only UX and confirmation changes

## Test Plan

- Unit tests for config interpolation and redaction
- Unit tests for read-only guardrails
- Integration tests with Docker MongoDB playground
- Manual smoke test:
  - Run TUI in read-only mode
  - Attempt insert/edit/delete (should be blocked)
  - Run CLI query/agg

## Next Steps

1) Make TUI data loads non-blocking and add loading states
2) Validate TUI UX with slow network simulations

## Documentation Updates

- `SPEC.md`: update config schema, read-only behavior, exit codes
- `PLAYGROUND.md`: clarify integration test steps
- Sample configs: include env var and read-only examples
