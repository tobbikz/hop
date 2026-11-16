# Contributing to Hop

Thanks for taking the time to contribute.

## Before you start

- Search existing issues and pull requests.
- For larger changes, open an issue first so we can agree on the approach.
- Security issues: follow [SECURITY.md](SECURITY.md), do not open a public issue.

## Development setup

```bash
git clone https://github.com/tobbikz/hop.git
cd hop
cargo build
```

You need Rust 1.85+ and an OpenSSH client. `sshpass` and a FreeRDP/Remmina client are only needed to exercise password logins and RDP profiles.

## Checks

Run these before pushing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI uses stable Rust and runs the same commands on Linux and macOS. Heavy optional tests (DuckDB) only run on manual `workflow_dispatch`.

## Code style

- `cargo fmt` is the source of truth; no style debates.
- Clippy must be warning-free; do not add `#[allow(...)]` without a reason in the PR.
- Keep new comments rare and factual.
- Prefer small, focused commits.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add keyring-backed password storage
fix: handle empty SSH host
docs: document --doctor output
chore: bump dependencies
```

## Pull requests

- One logical change per PR.
- Describe the user-visible effect and how you tested it.
- Update `CHANGELOG.md` under `Unreleased` for user-facing changes.
- Keep `README.md` and `docs/` accurate if behavior or paths change.

## Project scope

The product is an SSH/RDP session manager. The workspace contains only the crates needed for that: `ide-app`, `ide-ui`, `ide-sessions`, `ide-term`, `ide-config` and `ide-core`. Keep editor, database, language-server and agent features out of this repository.
