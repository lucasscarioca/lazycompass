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
- `.sha256.sig` signatures are optional and only meaningful if a maintainer has published a matching public key and fingerprint.
- Until a stable public signing key is published in this repo, treat checksum verification as the required release integrity check.
- If signing is enabled for a release, the public key location and fingerprint must be added here before that release is announced.

## Security review expectations

- Changes touching dependencies, install/upgrade flow, config persistence, filesystem writes, logging redaction, or Mongo write safety need explicit review before release.
