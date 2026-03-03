# Release process

1) Update versions in all crate `Cargo.toml` files.
2) Update [CHANGELOG.md](./CHANGELOG.md) with the release date and notes.
3) Run checks:

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

4) Build the workspace:

```bash
cargo build --workspace
```

5) Tag the release:

```bash
git tag -a v<version> -m "v<version>"
```

Use the same version in crate manifests, the tag, and the release notes.
The release workflow validates this and publishes the matching `CHANGELOG.md` section as the GitHub release notes.

Security hardening: for releases that change dependencies, config, or security-sensitive behavior, do an explicit security review before publishing.
