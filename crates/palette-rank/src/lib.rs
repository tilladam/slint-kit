//! Fuzzy ranking for command palettes.
//!
//! [`rank`] matches a query against each entry's text with nucleo-matcher
//! (subsequence matching, case-insensitive, smart Unicode normalisation)
//! and returns the matches most relevant first, capped at `limit`. An empty
//! query keeps the entries in their given order.
//!
//! Extracted from slinty-pi (`crates/pi-core/src/palette.rs`).

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// The cap slinty-pi uses: enough to scroll, few enough to stay cheap.
pub const DEFAULT_LIMIT: usize = 60;

struct Candidate {
    idx: usize,
    haystack: String,
}

impl AsRef<str> for Candidate {
    fn as_ref(&self) -> &str {
        &self.haystack
    }
}

/// `entries` ranked against `query`, most relevant first, at most `limit`.
/// `text` gives what an entry is matched on (e.g. its label and detail
/// joined by a space).
/// Equal scores retain input order. Matching is synchronous; for large catalogs,
/// call from a worker thread. A zero limit returns immediately without matching.
///
/// ```
/// let entries = ["Open file", "Close window"];
/// let matches = palette_rank::rank(&entries, "opfi", 10, |s| s.to_string());
/// assert_eq!(matches, ["Open file"]);
/// ```
pub fn rank<T: Clone>(
    entries: &[T],
    query: &str,
    limit: usize,
    text: impl Fn(&T) -> String,
) -> Vec<T> {
    if limit == 0 {
        return Vec::new();
    }
    if query.trim().is_empty() {
        return entries.iter().take(limit).cloned().collect();
    }
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let candidates = entries.iter().enumerate().map(|(idx, e)| Candidate {
        idx,
        haystack: text(e),
    });
    pattern
        .match_list(candidates, &mut matcher)
        .into_iter()
        .take(limit)
        .map(|(c, _score)| entries[c.idx].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_ties_and_limits() {
        let entries = [entry("a", "Café"), entry("b", "Café"), entry("c", "Other")];
        assert_eq!(
            r(&entries, "CAFE").iter().map(|e| e.id).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(rank(&entries, "cafe", 1, |e| e.label.into())[0].id, "a");
        assert!(rank(&entries, "cafe", 0, |_| panic!("text evaluated")).is_empty());
        assert!(rank::<Entry>(&[], "cafe", 10, |_| panic!("text evaluated")).is_empty());
        assert_eq!(r(&entries, "  \n "), entries);
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Entry {
        id: &'static str,
        label: &'static str,
        detail: &'static str,
    }

    fn entry(id: &'static str, label: &'static str) -> Entry {
        Entry {
            id,
            label,
            detail: "",
        }
    }

    fn r(entries: &[Entry], query: &str) -> Vec<Entry> {
        rank(entries, query, DEFAULT_LIMIT, |e| {
            format!("{} {}", e.label, e.detail)
        })
    }

    #[test]
    fn empty_query_returns_build_order() {
        let entries = vec![entry("a", "New session"), entry("b", "/compact")];
        assert_eq!(r(&entries, ""), entries);
    }

    #[test]
    fn fuzzy_query_matches_subsequence_and_ranks_closer_matches_first() {
        let entries = vec![
            entry("action:new-session", "New session"),
            entry("command:compact", "/compact"),
            entry("session:x", "unrelated title"),
        ];
        let ranked = r(&entries, "nsess");
        assert_eq!(ranked[0].id, "action:new-session");
        assert!(ranked.iter().all(|e| e.id != "session:x"));
    }

    #[test]
    fn non_matching_query_drops_entries() {
        let entries = vec![entry("a", "New session")];
        assert!(r(&entries, "zzzzz").is_empty());
    }

    #[test]
    fn detail_text_counts_and_limit_caps() {
        let entries = vec![
            Entry {
                id: "model:0",
                label: "qwen3.5-4b · rapid-mlx · free · local",
                detail: "load model",
            },
            entry("action:abort", "Abort"),
        ];
        assert_eq!(
            r(&entries, "load qwen").first().map(|e| e.id),
            Some("model:0")
        );
        assert_eq!(rank(&entries, "", 1, |e| e.label.to_string()).len(), 1);
    }
}
