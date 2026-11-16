# Sessions architecture (TECH-TUI-SESS-V1-ARCH)

## Crate
- `ide-sessions` — profiles + connect helpers
- Reuses `ide-term::TerminalPane::spawn_command` for SSH
- RDP: `std::process::Command` → `xfreerdp` / `wlfreerdp` / `remmina` (`which`)

## Store
`~/.config/hop/sessions.toml` — list of profiles (hosts, keys, jump). **No passwords in this file.**

Existing installs that only have `~/.config/tui-ide/` keep using it until the new directory exists; nothing is moved or deleted automatically.

Optional saved passwords (MobaXterm-style): `~/.config/hop/sessions-secrets.toml` (mode **0600**), keyed by profile id.

On SSH connect:
- with `identity_file` → connects straight to the PTY (`ssh -i`)
- without → password modal: **Enter** empty uses ssh-agent / default keys; typed password uses **`sshpass`** when available; **Tab** toggles remember. Clear via palette **Clear saved SSH password**.

```bash
sudo pacman -S sshpass   # Arch — recommended for password logins
```

## Profile fields
| Field | SSH | RDP |
|-------|-----|-----|
| name, host, user, port | ✓ | ✓ |
| identity_file | ✓ | — |
| jump_host (`ssh -J`) | ✓ | — |
| gateway | — | ✓ |
| rdp_client preference | — | ✓ |

## Host chrome
- Product boots into **Sessions** (Profiles list + Session main pane)
- Soft selection (`▎` + `surface_3`), rounded borders, theme tokens
- `Space` shortcuts · `Ctrl-P` palette · `Shift-T` theme info (Hop mono is locked)
- SSH live sessions → main pane PTY with multi-tabs (`]` / `[` / `Ctrl-W` in the profile list)
- RDP → external window; live/failed dot on the profile row
- Inside the PTY most keys go to the remote program; `Esc`, `Ctrl-\``, or `F12` returns to profiles
- `x` asks for confirmation before deleting a profile

## Non-goals
RDP/VNC framebuffer in ratatui, mosh, Termius clone.
