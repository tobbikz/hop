# Changelog

All notable changes to this project are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [0.6.1] - 2026-11-15

### Changed

- SSH PTY now receives every key except `Ctrl-\`` / `F12` (vim, readline and tmux work normally); tab actions live in the profile list.
- Profile rows lead with the name and target, with connected/failed dots; kind is only shown for RDP.
- Status line auto-clears after 4 seconds and uses success/error icons.
- Single locked AMOLED theme with improved faint-text contrast and one-column PTY padding.

### Fixed

- Deleting a profile asks for confirmation (`Enter`/`y` deletes, `Esc`/`n` cancels).
- Password fields accept spaces; `Tab` toggles "remember".
- SSH tab titles use the profile name (no `[[ssh] …]`), and the profile count is pluralized.

## [0.6.0] - 2026-11-15

### Added

- `--version`, `--help` and `--doctor` CLI flags.
- CI now checks formatting, clippy (`-D warnings`), tests and release builds on Linux and macOS.
- Tag-driven release workflow that publishes prebuilt Linux and macOS binaries.
- Community files: contributing guide, security policy, changelog, issue and PR templates, Dependabot.

### Changed

- New installs use `~/.config/tui-sessions`; existing `~/.config/tui-ide` directories keep working as a fallback.
- Session secret files are created with mode `0600` from the first write.
- Missing external tools (`ssh`, `sshpass`, RDP clients) now produce actionable error messages.
- The UI is sessions-only: the profile list, live PTY pane, SSH tabs, command palette and Space discover overlay.

### Removed

- Legacy IDE/database crates (`ide-db`, `ide-nvim`, `ide-lsp`, `ide-dap`, `ide-search`, `ide-agents`, `ide-plugins`, `ide-syntax`, `ide-fs`, `ide-git`, `ide-buffer`) and the workspace-directory argument.
- Editor, database, language-server, debugger and agent features from `ide-app`, `ide-ui`, `ide-core` and `ide-config`.

### Fixed

- Clippy warnings across the workspace.
- Generated SQL exports and ad-hoc queries are no longer tracked by git.

## [0.5.0] - 2026-11-04

### Added

- Initial tui-sessions product extract from tui-ide: SSH PTY tabs, RDP launch, profiles, themes, Space discover and MobaXterm-style optional password saving.
