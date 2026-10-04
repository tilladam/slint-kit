//! Shared `.slint` components for desktop apps, as a Slint component library.
//!
//! The crate is only files: add it as a **build-dependency** and register
//! the library in `build.rs`:
//!
//! ```ignore
//! let config = slint_build::CompilerConfiguration::new()
//!     .with_library_paths(slint_widgets::library_paths());
//! slint_build::compile_with_config("ui/app.slint", config).unwrap();
//! ```
//!
//! then import from it in `.slint`:
//!
//! ```slint,ignore
//! import { KitStyle, CopyButton, CodeBlock, CodeLine } from "@slint-widgets";
//! export { KitStyle } // so Rust can override its properties
//! ```
//!
//! `KitStyle` derives its colours from std-widgets' `Palette`, so light and
//! dark mode work without the app doing anything; an app can still override
//! any property from Rust (`app.global::<KitStyle>().set_mono_font(..)`, e.g.
//! with [`default_mono_font`] off macOS).
//!
//! Components extracted from slinty-pi (`slint/slinty-pi/ui/app.slint`).

use std::collections::HashMap;
use std::path::PathBuf;

/// The name to import from: `"@slint-widgets"`.
pub const LIBRARY_NAME: &str = "slint-widgets";

/// The library's entry file (`ui/lib.slint`), which exports everything.
pub fn library_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/lib.slint"))
}

/// `{ "slint-widgets": library_path() }`, ready for `with_library_paths`.
pub fn library_paths() -> HashMap<String, PathBuf> {
    HashMap::from([(LIBRARY_NAME.to_owned(), library_path())])
}

/// A monospace font that normally exists on this platform, for
/// `KitStyle.mono-font` (whose own default, Menlo, only exists on macOS).
pub fn default_mono_font() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else if cfg!(target_os = "windows") {
        "Consolas"
    } else {
        "DejaVu Sans Mono"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_entry_file_exists() {
        assert!(super::library_path().is_file());
    }
}
