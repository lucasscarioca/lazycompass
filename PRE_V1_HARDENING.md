# Pre-v1 Hardening Summary

This document summarizes security and safety hardening completed during the `0.9.x` stabilization phase.

It is not a `1.0` release note and does not indicate release readiness by itself.

## Completed hardening

- Storage: reject symlinked config/spec paths, use atomic temp-file writes, and keep repo/global `.env` resolution isolated.
- CLI: secure temp-file creation for editor flows and stop executing cwd `install.sh` during `lazycompass upgrade`.
- Installer: restrict remote installer downloads to GitHub raw repo paths and require release checksums.
- Output: neutralize CSV formula cells during export/copy.
- Query safety: cap query and aggregation result sets at 10,000 documents.
- Config: restrict `logging.file` to paths under the global config directory.
- Connection policy: reject insecure Mongo connections by default unless `--allow-insecure` is set.
- TUI: avoid full rendering of oversized documents and indexes; show bounded summaries instead.
- Dependencies: update `time` to `0.3.47` to clear `RUSTSEC-2026-0009`.

## Current verification

Local verification completed on `main` after the hardening work:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace
cargo test --workspace
cargo audit
```

## Remaining pre-v1 work

- Run the remaining release-readiness checks before cutting any `1.0` tag.
- Keep `cargo-audit` green in CI and re-run manual security review for any installer, dependency, config, or write-safety changes.
