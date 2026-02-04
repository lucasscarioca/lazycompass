# LazyCompass Phase 7 Plan

This plan focuses on Phase 7 (Production Readiness) from `SPEC.md`.

Status: planned (2026-02-04)

## Phase 7 — Production Readiness

Goal: Make LazyCompass safe and reliable for production usage with secure connections,
explicit safety guardrails, and operational readiness.

### Scope

1) Secure connections and secrets handling
2) Safety-first workflows and read-only mode
3) Reliability and operational observability
4) CI gating + integration testing against the MongoDB playground

## Workstreams and Tasks

### 7.1 Security and Secrets

Status: in progress

Scope:
- Support MongoDB URI features used in production (TLS/SRV/auth options)
- Allow env var interpolation for config values
- Ensure secrets are never logged

Tasks:
- [x] Add env var interpolation to config loading
  - Resolve `${VAR}` placeholders in config values (connections.uri, logging.file)
  - Define behavior when env var is missing (error + clear message)
- [ ] Introduce explicit redaction for connection URIs
  - Mask password/credentials in logs and error messages
  - Keep redaction consistent for TUI + CLI paths
- [ ] Document secret handling rules in `SPEC.md`

Deliverables:
- Config can safely reference `${MONGO_URI}`
- Logs never include passwords or tokens
- Updated spec and examples

### 7.2 Safety and Guardrails

Status: planned

Scope:
- Default to read-only unless explicitly enabled
- Clear safety cues for destructive operations

Tasks:
- Add a read-only toggle in config and CLI flag
  - CLI: `--read-only` (or `--write-enabled` with default off)
  - TUI: visible banner/status when read-only is active
- Block write operations in read-only mode
  - Insert/edit/delete in TUI
  - CLI write paths (future: inline writes)
- Improve destructive confirmations
  - Show context (connection, db, collection, _id)
  - Require explicit confirmation keyword for deletes (ex: type `delete`)

Deliverables:
- Read-only mode enforced in all write paths
- Stronger confirmation UX

### 7.3 Reliability and Performance

Status: planned

Scope:
- Add timeouts for network operations
- Improve handling of transient errors

Tasks:
- Add connect and query timeout configuration
  - Expose defaults in config
  - Ensure timeouts are applied in mongo executor
- Provide retry guidance or automatic retry for safe operations
  - Document behavior and limits
- Improve error surfaces
  - CLI: clear error messages + exit codes
  - TUI: visible status and non-blocking UI

Deliverables:
- Timeouts enforced for connect/query
- Clear and consistent error handling

### 7.4 Observability and Operations

Status: planned

Scope:
- Structured logs with consistent fields
- Log rotation or size limiting
- Consistent exit codes

Tasks:
- Add structured logging fields
  - command, component, connection name (redacted), database, collection
- Implement log file rotation or max-size cap
  - Define retention policy in config
- Define exit codes for CLI
  - Success, user error, config error, network error

Deliverables:
- Structured logs suitable for ops
- Controlled log size
- Documented exit codes

### 7.5 CI and Testing

Status: planned

Scope:
- CI gating for fmt/clippy/tests
- Integration tests using the Docker MongoDB playground

Tasks:
- Add CI workflow
  - Run `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`
- Add integration tests
  - Use docker-compose playground from `PLAYGROUND.md`
  - Validate connection + query + aggregation flows
- Add tests for read-only mode and env interpolation

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

## Documentation Updates

- `SPEC.md`: update config schema, read-only behavior, exit codes
- `PLAYGROUND.md`: clarify integration test steps
- Sample configs: include env var and read-only examples
