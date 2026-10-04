//! Bring a Slint [`VecModel`] to a wanted list of rows with row operations
//! instead of a reset, so a `ListView` keeps its scroll position and only
//! changed rows are rebuilt.
//!
//! Each row has a key (its identity) and a version (anything that changes
//! when the row must be rebuilt: an edit, a flag, a neighbour that changes
//! its rendering). [`KeyedModel::apply`] walks the current and the wanted
//! lists in order:
//!
//! - same key, same version: untouched;
//! - same key, new version: one `set_row_data`;
//! - a key that is no longer wanted: one `remove`;
//! - a new key: one `insert`, at its place;
//! - a key that moved: removed and inserted again where it now belongs.
//!
//! Rows are only built (the `build` callback) for inserts and updates.
//!
//! Generalised from yapper's timeline (`crates/yapper-ui/src/timeline.rs`,
//! `Timeline::apply`).

use std::collections::HashSet;
use std::hash::Hash;
use std::rc::Rc;

use slint::{Model, ModelRc, VecModel};

/// What [`KeyedModel::apply`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub inserted: usize,
    pub removed: usize,
    pub updated: usize,
}

/// A [`VecModel`] plus the key and version of each of its rows.
pub struct KeyedModel<K, T> {
    model: Rc<VecModel<T>>,
    entries: Vec<(K, u64)>,
}

impl<K, T> Default for KeyedModel<K, T>
where
    T: Clone + 'static,
{
    fn default() -> Self {
        Self {
            model: Rc::new(VecModel::default()),
            entries: Vec::new(),
        }
    }
}

impl<K, T> KeyedModel<K, T>
where
    K: Eq + Hash + Clone,
    T: Clone + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    /// The model to hand to Slint (e.g. a `ListView`'s `model`).
    pub fn model_rc(&self) -> ModelRc<T> {
        ModelRc::from(self.model.clone())
    }

    /// The underlying model, e.g. for `row_data`. Change rows only through
    /// [`apply`](Self::apply), or keys and rows drift apart.
    pub fn model(&self) -> &Rc<VecModel<T>> {
        &self.model
    }

    /// Keys in row order.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// Row index of a key.
    pub fn index_of(&self, key: &K) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    /// Brings the model to `want` (keys in row order, each with its
    /// version). `build` makes the row for a key; it is called only for
    /// inserted and updated rows. Keys in `want` must be unique.
    pub fn apply(&mut self, want: &[(K, u64)], mut build: impl FnMut(&K) -> T) -> Changes {
        debug_assert_eq!(
            want.iter().map(|(k, _)| k).collect::<HashSet<_>>().len(),
            want.len(),
            "keys in `want` must be unique"
        );
        let wanted: HashSet<&K> = want.iter().map(|(k, _)| k).collect();
        let current: HashSet<K> = self.entries.iter().map(|(k, _)| k.clone()).collect();
        let mut changes = Changes::default();
        let mut i = 0; // position in self.entries / the model
        let mut j = 0; // position in want
        while i < self.entries.len() || j < want.len() {
            match (self.entries.get(i), want.get(j)) {
                (Some(cur), Some(w)) if cur.0 == w.0 => {
                    if cur.1 != w.1 {
                        self.model.set_row_data(i, build(&w.0));
                        self.entries[i] = w.clone();
                        changes.updated += 1;
                    }
                    i += 1;
                    j += 1;
                }
                (Some(cur), _) if !wanted.contains(&cur.0) => {
                    self.model.remove(i);
                    self.entries.remove(i);
                    changes.removed += 1;
                }
                (None, Some(w)) => {
                    self.model.insert(i, build(&w.0));
                    self.entries.insert(i, w.clone());
                    changes.inserted += 1;
                    i += 1;
                    j += 1;
                }
                (Some(_), Some(w)) if !current.contains(&w.0) => {
                    self.model.insert(i, build(&w.0));
                    self.entries.insert(i, w.clone());
                    changes.inserted += 1;
                    i += 1;
                    j += 1;
                }
                (Some(_), Some(_)) => {
                    // Same keys in a different order (a move): drop the
                    // current row, it is inserted again where it belongs.
                    self.model.remove(i);
                    self.entries.remove(i);
                    changes.removed += 1;
                }
                (Some(_), None) => {
                    self.model.remove(i);
                    self.entries.remove(i);
                    changes.removed += 1;
                }
                (None, None) => break,
            }
        }
        debug_assert_eq!(self.entries.len(), self.model.row_count());
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Rows = KeyedModel<String, String>;

    fn want(items: &[(&str, u64)]) -> Vec<(String, u64)> {
        items.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    /// Applies `w`, building rows as "key@version", and checks the model.
    fn sync(m: &mut Rows, w: &[(&str, u64)]) -> Changes {
        let w = want(w);
        let versions: std::collections::HashMap<_, _> = w.iter().cloned().collect();
        let changes = m.apply(&w, |k| format!("{k}@{}", versions[k]));
        let rows: Vec<String> = m.model().iter().collect();
        let expected: Vec<String> = w.iter().map(|(k, v)| format!("{k}@{v}")).collect();
        assert_eq!(rows, expected);
        assert_eq!(
            m.keys().cloned().collect::<Vec<_>>(),
            w.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()
        );
        changes
    }

    fn ch(inserted: usize, removed: usize, updated: usize) -> Changes {
        Changes {
            inserted,
            removed,
            updated,
        }
    }

    #[test]
    fn refresh_is_row_operations_not_a_reset() {
        let mut m = Rows::new();
        assert_eq!(sync(&mut m, &[("a", 0), ("b", 0)]), ch(2, 0, 0));
        // Append: one insert, nothing else touched.
        assert_eq!(sync(&mut m, &[("a", 0), ("b", 0), ("c", 0)]), ch(1, 0, 0));
        // A new version: one update.
        assert_eq!(sync(&mut m, &[("a", 0), ("b", 1), ("c", 0)]), ch(0, 0, 1));
        // Delete in the middle: one remove.
        assert_eq!(sync(&mut m, &[("a", 0), ("c", 0)]), ch(0, 1, 0));
        // Older rows arriving above: inserts at the top only.
        assert_eq!(sync(&mut m, &[("z", 0), ("a", 0), ("c", 0)]), ch(1, 0, 0));
        // Nothing changed: nothing done.
        assert_eq!(sync(&mut m, &[("z", 0), ("a", 0), ("c", 0)]), ch(0, 0, 0));
        assert_eq!(m.index_of(&"c".to_owned()), Some(2));
    }

    #[test]
    fn a_move_is_a_remove_and_an_insert() {
        let mut m = Rows::new();
        sync(&mut m, &[("a", 0), ("b", 0), ("c", 0)]);
        assert_eq!(sync(&mut m, &[("b", 0), ("c", 0), ("a", 0)]), ch(1, 1, 0));
    }

    #[test]
    fn rows_are_built_only_for_inserts_and_updates() {
        let mut m = Rows::new();
        sync(&mut m, &[("a", 0), ("b", 0), ("c", 0)]);
        let mut built = Vec::new();
        m.apply(&want(&[("a", 0), ("b", 1), ("c", 0), ("d", 0)]), |k| {
            built.push(k.clone());
            k.clone()
        });
        assert_eq!(built, ["b", "d"]);
    }

    /// For random old/new key lists (drops, version bumps, moves), the model
    /// always ends exactly as wanted, and the counts add up.
    #[test]
    fn random_lists_converge() {
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let keys: Vec<String> = (0..12).map(|i| format!("k{i}")).collect();
        let mut m = Rows::new();
        for _ in 0..2000 {
            let mut w: Vec<(&str, u64)> = Vec::new();
            for k in &keys {
                if next(3) != 0 {
                    w.push((k.as_str(), next(2)));
                }
            }
            // Shuffle a little so moves happen too.
            for _ in 0..next(3) {
                if w.len() > 1 {
                    let (a, b) = (next(w.len() as u64) as usize, next(w.len() as u64) as usize);
                    w.swap(a, b);
                }
            }
            let before = m.model().row_count();
            let c = sync(&mut m, &w);
            assert_eq!(before + c.inserted - c.removed, w.len());
        }
    }
}
