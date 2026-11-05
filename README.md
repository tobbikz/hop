# tui-sessions

Terminal **Sessions** manager — SSH into a host PTY, RDP via external client.

Spun out of [tui-ide](https://github.com/tobbikz/tui-ide) as a focused product (no code editor / database studio).

## Run

```bash
cargo run -p ide-app --release
# binary: target/release/tui-sessions
```

## Keys

| Key | Action |
|-----|--------|
| `Enter` | Connect selected |
| `a` | Add SSH |
| `A` | Add RDP |
| `e` | Edit |
| `x` | Delete |

Config: `~/.config/tui-ide/sessions.toml` (shared path for now).

## License

Apache-2.0
