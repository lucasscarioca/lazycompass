# Release process

1) Update versions in all crate `Cargo.toml` files.
2) Update `CHANGELOG.md` with the release date and notes.
3) Run checks:

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

4) Build the workspace:

```bash
cargo build --workspace
```

5) Tag the release:

```bash
git tag -a v0.5.0 -m "v0.5.0"
```
