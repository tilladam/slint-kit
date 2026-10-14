//! Bring a Slint [`VecModel`] to a wanted list of rows with row operations
//! instead of a reset, so a `ListView` keeps its scroll position and only
//! changed rows are rebuilt.
//!
//! Each row has a key (its identity) and a version (anything that changes
//! when the row must be rebuilt: an edit, a flag, a neighbour that changes
//! its rendering). [`KeyedModel::apply`] retains a longest ordered sequence
//! of existing keys and reconciles the remaining rows:
//!
//! - same key, same version: untouched;
//! - same key, new version: one `set_row_data`;
//! - a key that is no longer wanted: one `remove`;
//! - a new key: one `insert`, at its place;
//! - a key that moved: removed and inserted again where it now belongs.
//!
//! Rows are only built for new keys and changed versions; moves reuse row data.
//! Sequence selection is O(n log n); VecModel inserts/removes can still shift
//! O(n) rows per operation. Models handed to consumers are read-only.
//!
//! ```
//! use slint_model_sync::KeyedModel;
//! use slint::Model;
//! let mut rows = KeyedModel::<&str, String>::new();
//! rows.apply(&[("a", 0), ("b", 0)], |key| key.to_string());
//! let handle = rows.model_rc();
//! rows.apply(&[("b", 0), ("a", 0)], |_| panic!("unchanged row"));
//! assert_eq!(handle.row_data(0).as_deref(), Some("b"));
//! ```
//!
//! Generalised from yapper's timeline (`crates/yapper-ui/src/timeline.rs`,
//! `Timeline::apply`).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use slint::{Model, ModelRc, ModelTracker, VecModel};

/// What [`KeyedModel::apply`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub inserted: usize,
    pub removed: usize,
    pub updated: usize,
}

/// A [`VecModel`] plus the key and version of each of its rows.
struct ReadOnly<T> {
    inner: VecModel<T>,
}

impl<T: Clone + 'static> Model for ReadOnly<T> {
    type Data = T;
    fn row_count(&self) -> usize {
        self.inner.row_count()
    }
    fn row_data(&self, row: usize) -> Option<T> {
        self.inner.row_data(row)
    }
    fn model_tracker(&self) -> &dyn ModelTracker {
        self.inner.model_tracker()
    }
    fn set_row_data(&self, _row: usize, _data: T) {}
}

/// Owns keyed, versioned rows and exposes a stable read-only Slint model.
/// Update rows through [`apply`](Self::apply); version changes invalidate data.
pub struct KeyedModel<K, T> {
    model: Rc<ReadOnly<T>>,
    entries: Vec<(K, u64)>,
}

impl<K, T> Default for KeyedModel<K, T>
where
    T: Clone + 'static,
{
    fn default() -> Self {
        Self {
            model: Rc::new(ReadOnly {
                inner: VecModel::default(),
            }),
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

    /// A read-only view for `row_data`, `row_count`, and iteration.
    /// Writes, including through [`model_rc`](Self::model_rc), are unsupported.
    pub fn model(&self) -> &impl Model<Data = T> {
        self.model.as_ref()
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
    /// new keys and changed versions. Moves with unchanged versions reuse rows.
    /// Keys in `want` must be unique (duplicates panic before mutation).
    /// Counts describe actual insert/remove/set operations, including moves.
    pub fn apply(&mut self, want: &[(K, u64)], mut build: impl FnMut(&K) -> T) -> Changes {
        assert_eq!(
            want.iter().map(|(k, _)| k).collect::<HashSet<_>>().len(),
            want.len(),
            "keys in `want` must be unique"
        );
        let positions: HashMap<&K, usize> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, (k, _))| (k, i))
            .collect();
        let sequence: Vec<usize> = want
            .iter()
            .filter_map(|(k, _)| positions.get(k).copied())
            .collect();
        let keep = retained_positions(&sequence);
        let wanted: HashSet<&K> = want.iter().map(|(k, _)| k).collect();
        let mut moved = HashMap::new();
        let mut changes = Changes::default();
        for i in (0..self.entries.len()).rev() {
            if !keep.contains(&i) {
                let (key, version) = &self.entries[i];
                if wanted.contains(key) {
                    moved.insert(
                        key.clone(),
                        (*version, self.model.inner.row_data(i).expect("tracked row")),
                    );
                }
                self.model.inner.remove(i);
                self.entries.remove(i);
                changes.removed += 1;
            }
        }
        for (i, (key, version)) in want.iter().enumerate() {
            if self.entries.get(i).is_some_and(|(k, _)| k == key) {
                if self.entries[i].1 != *version {
                    self.model.inner.set_row_data(i, build(key));
                    self.entries[i].1 = *version;
                    changes.updated += 1;
                }
            } else {
                let row = match moved.remove(key) {
                    Some((old_version, row)) if old_version == *version => row,
                    _ => build(key),
                };
                self.model.inner.insert(i, row);
                self.entries.insert(i, (key.clone(), *version));
                changes.inserted += 1;
            }
        }
        debug_assert_eq!(self.entries.len(), self.model.row_count());
        changes
    }
}

// Patience sorting. Replacing each tail with the smallest old position gives
// a deterministic LIS, favoring earlier old rows when sequences have equal length.
fn retained_positions(sequence: &[usize]) -> HashSet<usize> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; sequence.len()];
    for (i, &position) in sequence.iter().enumerate() {
        let slot = tails.partition_point(|&j| sequence[j] < position);
        if slot > 0 {
            previous[i] = Some(tails[slot - 1]);
        }
        if slot == tails.len() {
            tails.push(i);
        } else {
            tails[slot] = i;
        }
    }
    let mut retained = HashSet::new();
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        retained.insert(sequence[i]);
        cursor = previous[i];
    }
    retained
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_swap_reuses_rows_and_keeps_the_suffix() {
        let mut m = Rows::new();
        sync(&mut m, &[("a", 0), ("b", 0), ("c", 0)]);
        let changes = m.apply(&want(&[("b", 0), ("a", 0), ("c", 0)]), |_| {
            panic!("unchanged row rebuilt")
        });
        assert_eq!(changes, ch(1, 1, 0));
        assert_eq!(m.model().iter().collect::<Vec<_>>(), ["b@0", "a@0", "c@0"]);
    }

    #[test]
    fn consumer_cannot_modify_the_model() {
        let mut m = Rows::new();
        sync(&mut m, &[("a", 0)]);
        let handle = m.model_rc();
        handle.set_row_data(0, "bad".into());
        m.model().set_row_data(0, "bad".into());
        assert!(handle.as_any().downcast_ref::<VecModel<String>>().is_none());
        assert!(handle.insert_row(0, "bad".into()).is_err());
        assert!(handle.remove_row(0).is_err());
        assert_eq!(handle.row_data(0).as_deref(), Some("a@0"));
    }

    #[test]
    fn reverse_reuses_unchanged_rows_but_rebuilds_changed_versions() {
        let mut m = Rows::new();
        sync(&mut m, &[("a", 0), ("b", 0), ("c", 0), ("d", 0)]);
        let mut built = Vec::new();
        let c = m.apply(&want(&[("d", 0), ("c", 1), ("b", 0), ("a", 0)]), |k| {
            built.push(k.clone());
            format!("{k}@1")
        });
        assert_eq!(c, ch(3, 3, 0));
        assert_eq!(built, ["c"]);
        assert_eq!(
            m.model().iter().collect::<Vec<_>>(),
            ["d@0", "c@1", "b@0", "a@0"]
        );
    }

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
