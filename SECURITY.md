# Security policy

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Please do **not** open a public issue.

Use GitHub's private reporting flow:
[Security advisories](https://github.com/tobbikz/tui-sessions/security/advisories/new).

Include the affected version, platform, reproduction steps and impact. We aim to acknowledge reports within 72 hours.

## Handling of credentials

- `sessions.toml` never stores passwords.
- Optional saved passwords live in `sessions-secrets.toml` with mode `0600`.
- The secrets file is **not encrypted at rest**; it relies on local filesystem permissions. Do not use “remember password” on shared or untrusted machines.
- `ssh-agent` and identity files are the recommended authentication path.
- Logs in `~/.cache/tui-sessions/` never contain passwords.
