# md-segments

Split Markdown into blocks for native frontend renderers, highlight code into
colored spans, and transform inline links and emoji shortcodes. This crate is
independent of Slint and other UI toolkits.

```toml
[dependencies]
md-segments = "0.1.0"
```

```rust
use md_segments::segmenter::{Segment, segment_markdown};

let segments = segment_markdown("# Hello\n\nA paragraph.\n\n```rust\nlet n = 1;\n```");
assert!(matches!(&segments[0], Segment::Heading { level: 1, text } if text == "Hello"));

let lines = md_segments::highlight::highlight_lines("let n = 1;", "rust", false);
assert_eq!(lines.len(), 1);
```

Segments cover prose, headings, code, quotes, thematic breaks, and row-major
tables. Prose and quote segments carry the reference definitions their links
need to render independently. `inline` provides bare-URL linking, link
extraction, and emoji substitution in literal text. `segmenter::column_layout`
provides table column weights and an estimated natural width.

The default `highlight` feature uses syntect. To use only segmentation and inline
helpers, set `default-features = false`. Highlighting produces one `CodeLine`
per source line with RGB `ColoredSpan` values; it expands tabs to four spaces,
normalizes CRLF, and omits line endings. Preserve the original code for copying.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/md-segments). MIT licensed.
