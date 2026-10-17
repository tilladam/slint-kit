# palette-rank

Fuzzy ranking for command palettes using nucleo-matcher. Supply your own entry
type and searchable text; receive cloned entries in relevance order. This crate
does not depend on Slint.

```toml
[dependencies]
palette-rank = "0.1.0"
```

```rust
let entries = ["Open file", "Close window"];
let matches = palette_rank::rank(&entries, "opfi", 10, |entry| entry.to_string());
assert_eq!(matches, ["Open file"]);
```

Equal scores retain input order. An empty or whitespace-only query preserves
input order; the result limit applies to every query. Matching is synchronous,
so large catalogs can be ranked on a worker thread.

Run the example from a checkout with
`cargo run -p palette-rank --example rank`.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/palette-rank). MIT licensed.
