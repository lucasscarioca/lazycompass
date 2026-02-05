# Security review

Prior review (2026-02-05) flagged the items below.
Status: Done

## Findings and recommended mitigations

- [x] Read-only enforcement (Status: Done) — Finding: write actions can mutate data; Mitigation: default read-only mode with explicit opt-in for writes; Notes: guarded via read-only defaults and write-enabled flags.
- [x] $out / $merge stages (Status: Done) — Finding: aggregations can write to collections; Mitigation: block by default, allowlist when explicitly enabled; Notes: blocked unless read-only disabled and pipeline writes allowed.
- [x] TLS and authentication (Status: Done) — Finding: connections can be configured without TLS/auth; Mitigation: warn on insecure settings, prefer TLS/auth defaults; Notes: warnings on insecure connections with allow-insecure override.
- [x] Logging redaction (Status: Done) — Finding: logs may leak credentials or query data; Mitigation: redact URIs, auth, and sensitive fields; Notes: redaction added for sensitive log fields.
- [x] Config file permissions (Status: Done) — Finding: config/queries may be world-readable; Mitigation: enforce 0600/0700 and warn on permissive modes; Notes: enforced on write and checked on read.
- [x] Editor shell execution (Status: Done) — Finding: $EDITOR/$VISUAL may invoke shell; Mitigation: validate editor values, avoid shell invocation, document risk; Notes: command parsing only, no shell expansion.
- [x] curl|bash update path (Status: Done) — Finding: installer fetched over network; Mitigation: publish checksums/signatures and document verification steps; Notes: verification docs added, installer checksums/signatures supported.
