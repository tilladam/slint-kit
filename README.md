# slint-kit

Shared building blocks for desktop apps built with [Slint](https://slint.dev), extracted from
[slinty-pi](https://github.com/tilladam/slinty-pi) and yapper.

**Status:** early. The crates below exist and are tested in CI; no app uses them from here yet,
and APIs may still change.

## Rules

- **MIT.** Everything here is MIT-licensed (see `LICENSE`).
- **Public dependencies only.** Crates depend on Slint or other public crates from crates.io:
  no git dependencies, no other registries, and no path dependencies outside this repository.
  CI enforces this (`deny.toml` via `cargo deny`, plus a `cargo metadata` check).
- **Pure and Slint code stay apart.** Crates named `slint-*` depend on Slint. The others are
  toolkit-agnostic, usable from any frontend (slinty-pi's SwiftUI sibling included).
- **No AGPL code.** Everything here is original or comes from MIT-licensed sources.

## Crates

| Crate | Slint? | What it does |
|---|---|---|
| `md-segments` | no | Markdown → segments (prose, headings, code, quotes, tables), syntect highlighting (feature `highlight`), bare-URL linking and `:shortcode:` emoji |
| `emoji-shortcodes` | no | `:shortcode:` tables (iamcal, joypixels) |
| `desktop-clipboard` | no | Clipboard file lists, or images as their original encoded bytes; bounded `to_png` (feature `decoding-fallback` off macOS) |
| `desktop-notify` | no | macOS notifications with click-to-open by id, Dock badge, activate (feature `fallback`: notify-rust elsewhere) |
| `palette-rank` | no | Fuzzy ranking for command palettes (nucleo-matcher) |
| `slint-model-sync` | yes | Keyed, versioned `VecModel` reconcile that keeps `ListView` scroll positions |
| `slint-widgets` | yes | `.slint` component library (`@slint-widgets`): `KitStyle`, `CopyButton`, `CodeBlock`, `CommandPalette`, markdown blocks (`ProseBlock`, `HeadingBlock`, `QuoteBlock`, `RuleBlock`, `TableBlock`) |
| `slint-widgets-gallery` | yes | Not published: shows the widgets; `GALLERY_SCHEME=light\|dark`, `GALLERY_PALETTE=1` |

Planned: `slint-file-drop` (OS file drops with position).

## Using a crate

There are no tags or releases yet: pin a commit.

```toml
md-segments = { git = "https://github.com/tilladam/slint-kit", rev = "<commit>" }
```

`slint-widgets` is used as a **build-dependency**: pass `slint_widgets::library_paths()` to
`slint_build::CompilerConfiguration::with_library_paths` and `import { … } from "@slint-widgets"`.

Slint crates here follow the Slint minor version their consumers use (currently 1.18).
