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
| `slint-widgets` | yes | `.slint` component library (`@slint-widgets`): `KitStyle`, `CopyButton`, `ConfirmButton`, `CodeBlock`, `CommandPalette`, markdown blocks (`ProseBlock`, `HeadingBlock`, `QuoteBlock`, `RuleBlock`, `TableBlock`) |
| `slint-widgets-gallery` | yes | Not published: shows the widgets; `GALLERY_SCHEME=light\|dark`, `GALLERY_PALETTE=1` |
| `slint-widgets-tests` | yes | Not published: headless interaction and model integration tests |

Planned: `slint-file-drop` (OS file drops with position).

## Using a crate

There are no tags or releases yet: pin a commit.

```toml
md-segments = { git = "https://github.com/tilladam/slint-kit", rev = "<commit>" }
```

`slint-widgets` is used as a **build-dependency**: pass `slint_widgets::library_paths()` to
`slint_build::CompilerConfiguration::with_library_paths` and `import { … } from "@slint-widgets"`.

Slint crates here follow the Slint minor version their consumers use (currently 1.18).

## Examples and API contracts

Run the pure examples with `cargo run -p palette-rank --example rank` and
`cargo run -p slint-model-sync --example reconcile`. Native examples are compiled
in CI: `desktop-clipboard --example paste` reads a real clipboard, and
`desktop-notify --example notify` needs a supported notification configuration.

- **Markdown:** prose and quote segments include the reference definitions their
  links need, so each can be parsed independently. Highlighted display spans
  expand tabs to four spaces, normalize CRLF, and omit line endings. Keep the
  original code string for copying. Disable `highlight` to omit syntect.
- **Ranking:** equal scores retain input order, and an empty or whitespace-only
  query preserves build order. The limit applies to every query. Ranking is
  synchronous; move large catalogs off the UI thread.
- **Models:** retain the handle from `KeyedModel::model_rc()` and update through
  `apply`. Keys must be unique; duplicates panic before mutation. Increase the
  version whenever display data changes. Reorders retain a longest ordered
  sequence and reuse unchanged moved rows. `Changes` counts actual insert,
  remove, and set operations. The returned model is read-only, including through
  Slint two-way bindings; writes do not change it. Sequence selection is
  O(n log n), while row insertions and removals still shift vector contents.
- **Clipboard:** file lists take priority over alternate image representations.
  `None` lets the native text input handle paste. Recognized but unreadable
  content returns a typed error; `Error::source` preserves library failures.
  Decode images with `to_png` on a worker, then send the result to the event loop.
  Off macOS, images require `decoding-fallback`, which decodes before size checks.
- **Notifications:** `capabilities()` describes platform support, not permission
  or service availability. Install once, before the event loop; repeated
  installation returns `AlreadyInstalled`. Request authorization explicitly and
  query status through callbacks. `notify_with_result` reports request acceptance,
  not delivery. Callbacks can be synchronous or run on native threads; marshal
  UI updates onto the event loop. Fallback posting can block and belongs on a
  worker. Fallback supports showing notifications only: status is `Unknown`,
  permission requests are unsupported, and ids do not provide replacement,
  withdrawal, or click routing. Badge and activation operations are macOS-only;
  call them on the main thread. Unbundled macOS binaries report unsupported
  notification operations.

API migration from the original 0.1 implementation: `KeyedModel::model()` now
returns a read-only model view rather than `Rc<VecModel>`. Clipboard functions
return `desktop_clipboard::Error` instead of strings. Notification `install`
returns a result and no longer requests authorization automatically. The
fire-and-log `notify` convenience function remains available.

## Widget integration

Register `slint_widgets::library_paths()` from `build.rs`, as shown in the gallery,
and import components from `@slint-widgets`. Import and re-export `KitStyle` to
set its properties from Rust. For `default_mono_font()` in application code, add
`slint-widgets` as a regular dependency as well as a build dependency.

| Component | Inputs and sizing | Outputs and callbacks |
|---|---|---|
| `CopyButton` | `payload`; 34 × 20 logical px | `copy-requested(string)`; the app performs copying. Check mark lasts 1.4 s and repeated activation restarts it. Accessible label is Copy. |
| `ConfirmButton` | `text`, `confirm-text`, font sizes and text colors, `idle-width`, `confirm-width`, `outlined`, `timeout`; height 20 px | `armed`, `confirmed()`; first activation arms, second confirms. Timeout or focus loss disarms. |
| `CodeBlock` | `lang`, original `code`, nested `lines`, theme-matched `code-background`; give an available width, height follows content | `copy-requested(string)`; long lines scroll horizontally. |
| `CommandPalette` | `entries`, `placeholder-text`, `close-shortcut-key`; fill its parent as an overlay | `query-changed(string)`, `exec(string)`, `close()`; the app ranks entries and owns visibility. |
| `ProseBlock` | `styled-text` `text`, optional `copy-payload` | `open-link(string)`, `copy-requested(string)` |
| `HeadingBlock` | plain `text`, `level`, optional `copy-payload` | `copy-requested(string)` |
| `QuoteBlock` | `styled-text` `text`, optional `copy-payload` | `open-link(string)`, `copy-requested(string)` |
| `RuleBlock` | available width | No callbacks |
| `TableBlock` | row-major `rows`; explicit width, usually min(available width, natural width) | No callbacks; row heights follow wrapping cells. |

`PaletteRow` contains `id`, `icon`, `label`, and `detail`. `ColoredSpan` contains
`text` and `color`; `CodeLine` contains a model of `spans`. Convert the Rust
highlight output into nested `ModelRc<VecModel<...>>` values, as in the gallery.
For tables, convert each Rust `TableCell` into the Slint `TableCell` (`text`,
`header`, `weight`) and wrap rows in `TableRowCells`. Use the weights and natural
width from `md_segments::segmenter::column_layout` consistently across all rows.

Create the palette conditionally when opening it so its query starts empty and
receives focus. Up/Down keeps selection visible, Enter executes, and Escape or
the backdrop closes. `close-shortcut-key` uses the Meta modifier. The app must
restore focus on close/exec; the gallery demonstrates returning to its trigger.
Copy and confirmation controls are Tab-reachable, show focus outlines, and accept
Space/Enter as well as pointer and accessibility actions. Embedded copy controls
therefore participate in the enclosing application's Tab order.

`KitStyle` exposes `radius`, `mono-font`, `code-font-size`, `dim`, `faint`,
`hairline`, `hover`, `card`, and `accent`. Defaults follow Slint's Palette except
for Menlo; set `mono-font` using `default_mono_font()` off macOS. Highlighted
code colors need to be recomputed when the color scheme changes.

## Validation

CI checks formatting, all targets with Clippy, all-feature tests, strict rustdoc,
and pure examples on macOS, Linux, and Windows. It separately tests and documents
`md-segments`, `desktop-clipboard`, and `desktop-notify` without default features.
The gallery compiles the components; the unpublished test crate exercises input,
timers, accessibility actions, model updates, and Slint Markdown rendering without
a display. Its testing backend is pinned to Slint 1.18.1 and must be updated
together with Slint. Dependency source and license checks run on Linux.

Native smoke checks need a real desktop session:

1. Paste ordinary text, a file list, and PNG/JPEG/TIFF images. Confirm file lists
   win when the clipboard also offers images, pixels survive conversion, and
   unreadable or oversized data returns an error. On Linux/Windows, run the image
   example with `--features decoding-fallback`.
2. In a macOS app bundle with a bundle identifier, install before the event loop,
   request permission, inspect authorization status, and post a notification.
   Verify denial, replacement by id, grouping, click routing while running and
   after launch, withdrawal, and clearing the Dock badge at zero.
3. On Linux/Windows with a notification service, enable `fallback`, post from a
   worker, and verify accepted and failed posting outcomes. Click routing,
   withdrawal, and badges remain unsupported.
4. Run the gallery in both light and dark schemes, exercise the palette with more
   rows than fit, and check keyboard focus, long code lines, and wrapping tables.
