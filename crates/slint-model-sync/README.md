# slint-model-sync

Reconcile keyed, versioned rows into a read-only Slint model without resetting
the model. Unchanged rows are reused across reorders, preserving row identity
where possible and avoiding full model resets that disrupt views.

```toml
[dependencies]
slint-model-sync = "0.1.0"
slint = { version = "1.18", default-features = false, features = ["std", "compat-1-18"] }
```

```rust
use slint::Model;
use slint_model_sync::KeyedModel;

let mut rows = KeyedModel::<&str, String>::new();
rows.apply(&[("a", 0), ("b", 0)], |key| key.to_string());
let handle = rows.model_rc(); // Pass this stable handle to the Slint view.
rows.apply(&[("b", 0), ("a", 0)], |_| panic!("unchanged row rebuilt"));
assert_eq!(handle.row_data(0).as_deref(), Some("b"));
```

Keys must be unique; duplicates panic before mutation. Increase a row's version
whenever its display data changes. `apply()` retains a longest ordered sequence
of keys and reports actual insert, remove, and update counts. Moved rows with
unchanged versions reuse their data. Sequence selection is O(n log n); vector
insertions and removals can still shift O(n) rows per operation.

Update through `apply()`. Models returned by `model()` and `model_rc()` reject
external mutation. The crate enables Slint's model types without a backend or
renderer. Models stay on their owning UI thread.

Run `cargo run -p slint-model-sync --example reconcile` from a checkout.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/slint-model-sync). MIT licensed;
Slint has its own licensing terms.
