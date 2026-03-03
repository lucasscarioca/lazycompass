# Security Policy

## Supported releases

- The latest tagged release is the only supported stable release line.
- Unreleased `main` may change without notice and should not be treated as a supported security target.

## Reporting a vulnerability

- Preferred path: open a private GitHub security advisory at `https://github.com/lucasscarioca/lazycompass/security/advisories/new`
- If private reporting is unavailable, open a minimal GitHub issue asking for a private follow-up and do not include exploit details.
- Include: affected version, impact, reproduction steps, and whether the issue is already public.

## Release verification

- Release archives must ship with a `.sha256` checksum file.
- Signed releases use the public key in [keys/lazycompass-release-signing.asc](./keys/lazycompass-release-signing.asc).
- Current release signing fingerprint: `5D7E F1CB 7FD9 672A 6D11 3B5C 7450 2B60 9A66 0BAA`
- `0.9.0` predates signing setup and does not ship `.sha256.sig` assets.
- If a release ships both `.sha256` and `.sha256.sig`, verify the signature first, then verify the checksum against the archive.

## Security review expectations

- Changes touching dependencies, install/upgrade flow, config persistence, filesystem writes, logging redaction, or Mongo write safety need explicit review before release.
