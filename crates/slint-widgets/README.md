# slint-widgets

A Slint component library with a command palette, Markdown blocks, code views,
copy buttons, and confirmation buttons. The crate bundles the `.slint` sources
and exposes their library path; it has no Rust dependencies. Components target
Slint 1.18 or later compatible releases.

## Setup

Add the library to your application's build dependencies:

```toml
[dependencies]
slint = "1.18"

[build-dependencies]
slint-build = "1.18"
slint-widgets = "0.1.0"
```

Register the component library in `build.rs`:

```rust,ignore
fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(slint_widgets::library_paths());
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
```

Import the components in `ui/app.slint`:

```slint
import { CopyButton, KitStyle } from "@slint-widgets";
export { KitStyle }

export component App inherits Window {
    callback copy-requested(string);
    CopyButton {
        payload: "Hello";
        copy-requested(text) => { root.copy-requested(text); }
    }
}
```

The application handles `copy-requested`; components do not access the clipboard.

## Components

| Component | Purpose |
|---|---|
| `CopyButton` | Copy callback, temporary check mark, keyboard and accessibility activation |
| `ConfirmButton` | First activation arms, second confirms; timeout or focus loss disarms |
| `CommandPalette` | Query field and scrollable results; application supplies ranking and executes ids |
| `CodeBlock` | Colored spans, horizontal scrolling, and copying of original source |
| `ProseBlock`, `HeadingBlock`, `QuoteBlock`, `RuleBlock` | Markdown block presentation |
| `TableBlock` | Row-major cells with shared column weights and wrapping |

The library exports `PaletteRow`, `CodeLine`, `ColoredSpan`, `TableCell`, and
`TableRowCells` for model conversion. `KitStyle` follows Slint's Palette for
light and dark colors. Add `slint-widgets` as a regular dependency too if you
use `default_mono_font()` to set its monospace font from Rust.

Create `CommandPalette` conditionally when opening it. The app owns visibility,
handles `query-changed`, `exec`, and `close`, and restores focus on close or
execution. Copy and confirmation buttons participate in Tab navigation and
accept Space and Enter.

See the [integration reference](https://github.com/tilladam/slint-kit#widget-integration)
for properties, sizing, model conversions, and callbacks. The
[gallery](https://github.com/tilladam/slint-kit/tree/main/crates/slint-widgets-gallery)
is a complete consumer; run `cargo run -p slint-widgets-gallery` from a checkout.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[Rust API documentation](https://docs.rs/slint-widgets). MIT licensed;
Slint has its own licensing terms.
