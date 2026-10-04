//! Markdown for native desktop frontends whose inline renderer (Slint's
//! `StyledText`, SwiftUI's `AttributedString`) covers only inline markup.
//!
//! - [`segmenter`]: block-level split into prose, headings, code, quotes,
//!   rules and tables (from slinty-pi's `pi-render`).
//! - [`highlight`]: syntect highlighting into plain coloured spans (feature
//!   `highlight`, on by default; from slinty-pi's `pi-render`).
//! - [`inline`]: bare-URL linking, link extraction and `:shortcode:` emoji
//!   in literal text (from yapper).

#[cfg(feature = "highlight")]
pub mod highlight;
pub mod inline;
pub mod segmenter;
