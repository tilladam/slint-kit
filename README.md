# slint-kit

Shared building blocks for desktop apps built with [Slint](https://slint.dev), extracted from
[slinty-pi](https://github.com/tilladam/slinty-pi) and yapper.

**Status:** empty. Crates land one at a time; the repository stays private until the first
ones have settled.

## Rules

- **MIT.** Everything here is MIT-licensed (see `LICENSE`).
- **Public dependencies only.** Crates depend on Slint or other public crates from crates.io:
  no git dependencies, no other registries, and no path dependencies outside this repository.
  CI enforces this (`deny.toml` via `cargo deny`, plus a `cargo metadata` check).
- **Pure and Slint code stay apart.** Crates named `slint-*` depend on Slint. The others are
  toolkit-agnostic, usable from any frontend (slinty-pi's SwiftUI sibling included).
- **No copied AGPL code.** Ideas seen in AGPL projects are reimplemented
  from a description, never copied.

## Planned crates

| Crate | Slint? | What it does |
|---|---|---|
| `md-segments` | no | Markdown → segments (prose, headings, code, quotes, tables), syntect highlighting, linkify |
| `emoji-shortcodes` | no | `:shortcode:` tables (iamcal, joypixels) |
| `desktop-clipboard` | no | Clipboard images as their original encoded bytes |
| `desktop-notify` | no | Notifications and dock badge, if no existing crate fits |
| `slint-model-sync` | yes | Keyed `VecModel` reconcile that keeps `ListView` scroll positions |
| `slint-file-drop` | yes | OS file drops with hover state and drop position |
| `slint-widgets` | yes | `.slint` components: command palette, code block, copy button, tokens, SVG icons |
| `local-llm` | no | rapid-mlx / llama.cpp / Ollama management; OpenAI-compatible streaming client |

The plan, with sources, phases and acceptance checks, is `docs/plans/shared-crates.md` in
slinty-pi.

## Using a crate

Until crates are published, depend on a tag:

```toml
md-segments = { git = "https://github.com/tilladam/slint-kit", tag = "v0.1.0" }
```

Slint crates here follow the Slint minor version their consumers use (currently 1.18).
