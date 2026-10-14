use slint::Model;
use slint_model_sync::KeyedModel;

fn main() {
    let mut rows = KeyedModel::<&str, String>::new();
    let model = rows.model_rc(); // Hand this stable handle to Slint.
    rows.apply(&[("a", 0), ("b", 0), ("c", 0)], |key| key.to_string());
    let changes = rows.apply(&[("b", 0), ("a", 0), ("c", 0)], |_| {
        panic!("unchanged rows must be reused")
    });
    assert_eq!(model.iter().collect::<Vec<_>>(), ["b", "a", "c"]);
    assert_eq!((changes.inserted, changes.removed), (1, 1));
    println!("{changes:?}");
}
