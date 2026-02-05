# Security review

Prior review (2026-02-05) flagged the items below.
Status: Not started

## Findings and recommended mitigations

- [ ] Read-only enforcement (Status: Not started) — Finding: write actions can mutate data; Mitigation: default read-only mode with explicit opt-in for writes.
- [ ] $out / $merge stages (Status: Not started) — Finding: aggregations can write to collections; Mitigation: block by default, allowlist when explicitly enabled.
- [ ] TLS and authentication (Status: Not started) — Finding: connections can be configured without TLS/auth; Mitigation: warn on insecure settings, prefer TLS/auth defaults.
- [ ] Logging redaction (Status: Not started) — Finding: logs may leak credentials or query data; Mitigation: redact URIs, auth, and sensitive fields.
- [ ] Config file permissions (Status: Not started) — Finding: config/queries may be world-readable; Mitigation: enforce 0600/0700 and warn on permissive modes.
- [ ] Editor shell execution (Status: Not started) — Finding: $EDITOR/$VISUAL may invoke shell; Mitigation: validate editor values, avoid shell invocation, document risk.
- [ ] curl|bash update path (Status: Not started) — Finding: installer fetched over network; Mitigation: publish checksums/signatures and document verification steps.
