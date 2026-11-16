# Sessions architecture (TECH-TUI-SESS-V1-ARCH)

## Crate
- `ide-sessions` — profiles + connect helpers
- Reuses `ide-term::TerminalPane::spawn_command` for SSH
- RDP: `std::process::Command` → `xfreerdp` / `wlfreerdp` / `remmina` (`which`)

## Store
`~/.config/tui-sessions/sessions.toml` — list of profiles (hosts, keys, jump). **No passwords in this file.**

Existing installs that only have `~/.config/tui-ide/` keep using it until the new directory exists; nothing is moved or deleted automatically.

Optional saved passwords (MobaXterm-style): `~/.config/tui-sessions/sessions-secrets.toml` (mode **0600**), keyed by profile id.

On SSH connect (empty `identity_file`): password modal → **Space** toggles remember → **Enter** connects via **`sshpass`** when available (reliable, no passphrase-for-key confusion). Clear via palette **Clear saved SSH password**.

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
- `Space` discover · `Ctrl-P` palette · `Shift-T` theme info (AMOLED mono is locked)
- SSH live sessions → main pane PTY with multi-tabs (`Ctrl-]` / `Ctrl-W`)
- RDP → external window; live mark on profile row
- Chord `Ctrl-Shift-S` / palette **Sessions** still works from other surfaces

## Non-goals
RDP/VNC framebuffer in ratatui, mosh, Termius clone.
