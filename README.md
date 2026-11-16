# tui-sessions

**Keyboard-first SSH / RDP session manager for the terminal.**
Profiles on the left · live SSH PTY on the right · Space discover, soft selection — same visual language as [DB Studio](https://github.com/tobbikz/db-studio).

[![CI](https://github.com/tobbikz/tui-sessions/actions/workflows/ci.yml/badge.svg)](https://github.com/tobbikz/tui-sessions/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/tobbikz/tui-sessions?display_name=tag)](https://github.com/tobbikz/tui-sessions/releases)

Spun out of [tui-ide](https://github.com/tobbikz/tui-ide) as a focused product.

---

## Why

Desktop SSH clients are fine on a laptop. On a bastion, over SSH, or inside tmux you want something that:

- Starts in **Sessions**, not a fake IDE home
- Connects with **Enter** · add SSH/RDP without leaving the keyboard
- Uses **ssh-agent / identity files** — never stores passwords in `sessions.toml`
- Feels like a modern TUI: **soft selection**, **Space discover**, **AMOLED-mono theme**

RDP launches your local client (`xfreerdp` / `wlfreerdp` / `remmina`). SSH stays in-process as a PTY with multi-tabs.

---

## Requirements

| Requirement | Notes |
|---|---|
| Rust 1.85+ (building from source) | `rustup` recommended |
| Truecolor terminal | kitty, WezTerm, Alacritty, iTerm2, Windows Terminal, … |
| Nerd Font | Recommended for icons |
| OpenSSH client (`ssh`) | Required for SSH sessions |
| `sshpass` | Optional; automates password logins |
| FreeRDP or Remmina | Optional; required for RDP profiles |

Run `tui-sessions --doctor` at any time to see what is detected.

---

## Install

**Prebuilt binaries:** download the archive for your platform from
[Releases](https://github.com/tobbikz/tui-sessions/releases), unpack, and put `tui-sessions` on your `PATH`.

**From source:**

```bash
cargo install --git https://github.com/tobbikz/tui-sessions ide-app
```

or, for a local checkout:

```bash
git clone https://github.com/tobbikz/tui-sessions.git && cd tui-sessions
cargo run -p ide-app --release
# binary: target/release/tui-sessions
```

---

## Quick start

```bash
tui-sessions                # open the session manager
tui-sessions --doctor       # print environment diagnostics and exit
```

Press `a` for SSH, `A` for RDP, then `Enter` to connect.

---

## Keys

| Key | Action |
|-----|--------|
| `Enter` | Connect selected profile |
| `a` / `A` | Add SSH / Add RDP |
| `e` | Edit |
| `x` | Delete |
| `Space` | Discover next keys |
| `Ctrl-P` | Command palette |
| `Shift-T` | Theme info (AMOLED mono is locked) |
| `Ctrl-]` / `Ctrl-[` | Next / prev SSH tab |
| `Ctrl-W` | Close SSH tab |
| `Esc` | Close discover / form · back |

---

## Features

- **Profiles** — SSH + RDP, jump host (`-J`), identity file, RDP gateway
- **Live SSH** — multi-tab PTY in the main pane
- **RDP** — external client spawn; status in the list
- **Theme** — AMOLED mono, shipped as a single locked palette
- **Space discover** — contextual chord sheet
- **Soft selection** — accent bar + surface, no white-wash
- **Passwords like MobaXterm** — modal + optional remember (`sessions-secrets.toml`, mode 0600); uses `sshpass` when installed
- **Safe profiles** — `sessions.toml` never stores passwords

Full notes: [docs/SESSIONS.md](docs/SESSIONS.md)

---

## Configuration

```
~/.config/tui-sessions/sessions.toml          # profiles (no passwords)
~/.config/tui-sessions/sessions-secrets.toml  # optional saved passwords (0600)
~/.config/tui-sessions/config.toml            # theme setting
```

Legacy installs that only have `~/.config/tui-ide/` keep using that directory; nothing is moved automatically.
Logs: `~/.cache/tui-sessions/tui-sessions.log`.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| `tui-sessions: command not found` | Add the release binary or `~/.cargo/bin` to `PATH` |
| Colors look wrong | Use a truecolor terminal; `--doctor` shows `TERM` / `COLORTERM` |
| Password login keeps prompting | Install `sshpass` (see below) |
| “no RDP client found” | Install FreeRDP/Remmina or set `rdp_client` on the profile |
| “`ssh` was not found in PATH” | Install an OpenSSH client |

```bash
# Debian / Ubuntu
sudo apt install openssh-client sshpass freerdp2-x11
# Fedora
sudo dnf install openssh-clients sshpass freerdp
# Arch
sudo pacman -S openssh sshpass freerdp
```

Attach the log file when reporting a bug: `~/.cache/tui-sessions/tui-sessions.log`.

---

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

See [CONTRIBUTING.md](CONTRIBUTING.md). CI runs formatting, clippy, tests and a release build on Linux and macOS.

---

## License

Apache-2.0 — see [LICENSE](LICENSE).
