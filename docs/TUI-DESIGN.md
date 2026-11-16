# Hop TUI design system

Synthesizes four references stored in Brain (`product-hop` → uses):

| Source | What Hop takes |
|--------|----------------|
| [Monospace Design TUI](https://github.com/coreyt/monospace-design-tui) | Master–detail archetype, footer command bar, auditable focus/keyboard rules |
| [Charm](https://github.com/charmbracelet) | Restrained chrome, rounded borders, faint help, brand-first masthead |
| [tuile](https://github.com/Osyna/tuile) | Semantic roles (`text`, `muted`, `panel`, `boost`, `border`, `selection`, `footer_key`) |
| [jackin / Rivora](https://jackin.tailrocks.com/reference/tui/visual-design/) | Surface levels L0–L3, accent focus rail, single-line borders, contextual footer |

## Surfaces

| Level | Hop use |
|-------|---------|
| L0 | Terminal / app background (`bg`) |
| L1 | Header, profile list, session pane, footer (`surface`) |
| L2 | Selected row when focused, active tab (`surface_3` + accent rail) |
| L3 | Modals / discover / palette (`modal` + scrim) |

## Focus

- Exactly one interaction owner.
- Focused pane: accent border + brighter title.
- Unfocused: idle border, muted title, softened selection.
- List selection: left rail `▎` (never full reverse-video wash).

## Color (locked Hop mono — OpenCode-inspired)

- AMOLED pure black (`#000000`) like btop — panels/header/footer share the same black (no raised gray fills).
- Hierarchy via text brightness + thin borders only; selection = reverse or rail+bold.
- Accent = light gray/white; errors stay grayscale (bold `!`).
- Plain (square) borders; all paint through `theme::` tokens.

## Motion (restrained)

| Effect | Where | Notes |
|--------|-------|-------|
| Braille spinner `⠋⠙…` | only while `ui.busy` (connecting / launching) | 80 ms; off with `HOP_ANIMATIONS=0` |
| Live pulse | profile `●` brightness faint↔muted | slow (~900 ms); glyph stays `●` |

## Chrome rules

- Rounded single-line borders only.
- Brand `Hop` is the header hero; counts and state stay muted.
- Footer is a command bar: `key` (accent) + action (faint), mode-aware.
- Inside SSH PTY most keys go remote; host escape is `Esc` / `` Ctrl-` `` / `F12` → Profiles.
