# Release process

1) Update versions in all crate `Cargo.toml` files.
2) Update `CHANGELOG.md` with the release date and notes.
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
git tag -a v0.9.0 -m "v0.9.0"
```

Security hardening: for releases that change dependencies, config, or security-sensitive behavior, do an explicit security review before publishing.
