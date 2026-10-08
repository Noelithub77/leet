# Security policy

## Supported versions

Security fixes target the latest stable release and current `main`. Older releases do not receive a separate maintenance stream; upgrade to the latest stable build when a fix is published.

## Report a vulnerability

Use GitHub’s **[Report a vulnerability](https://github.com/Noelithub77/leet/security/advisories/new)** form to contact the maintainer privately. Do not post vulnerabilities, exploit details, tokens, account cookies, or personal solution data in public issues.

Include the affected version or commit, operating system, reproduction steps, expected impact, and a minimal proof of concept using test data. Redact credentials and personal paths. The maintainer will review the report and coordinate a fix and disclosure; this volunteer project does not promise a response deadline or a bug bounty.

## Relevant boundaries

Leet runs local solutions and configured agent tools with your user’s permissions; it is not a sandbox for untrusted programs. Review code and agent permissions before running them. Competitive Companion imports and provider statements are external input.

Release checksums detect corrupted downloads and are distributed with the release. They are not independent publisher signatures. Current builds are unsigned or ad-hoc signed as described in [release requirements](docs/releases.md); the installer does not bypass OS security controls.

For ordinary bugs and installation questions, use the [issue tracker](https://github.com/Noelithub77/leet/issues).
