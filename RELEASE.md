# Release process

1) Update versions in all crate `Cargo.toml` files.
2) Update [CHANGELOG.md](./CHANGELOG.md) with the release date and notes.
3) Run local checks:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

4) Build the workspace:

```bash
cargo build --workspace
```

5) Review release trust material:

- If signing is enabled, verify the release public key and fingerprint documented in [SECURITY.md](./SECURITY.md) are current.
- Ensure the automated installer and upgrader still verify signatures against the bundled LazyCompass release key, not the ambient user keyring.
- If signing is not enabled, do not claim signature verification in release notes.
- Release installs require `curl`, `tar`, and a SHA-256 tool (`sha256sum` or `shasum`). Keep that requirement documented when installer behavior changes.
- Windows stays beta until the release install/upgrade path is explicitly promoted; do not describe Windows self-upgrade as stable before that happens.

6) Tag the release:

```bash
git tag -a v<version> -m "v<version>"
```

Use the same version in crate manifests, the tag, and the release notes.
The release workflow re-runs Linux verify checks plus macOS/Windows build+test coverage on the tagged commit, creates a draft release, uploads artifacts and checksums, then publishes only after all build jobs succeed.

Security hardening: for releases that change dependencies, config, or security-sensitive behavior, do an explicit security review before publishing.
