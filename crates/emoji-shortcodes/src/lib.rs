//! Standard emoji name → code point tables, for chat servers that send
//! emoji by name only. Codes are Unicode code points in hex, joined by `-`.
//! Extracted from yapper (`crates/yapper-emoji`).
//!
//! - `data/iamcal.tsv`: iamcal/emoji-data (MIT,
//!   data/LICENSE-iamcal-emoji-data), the dataset Mattermost's names
//!   derive from.
//! - `data/joypixels.tsv`: joypixels emoji-toolkit 4.5.2 names, the
//!   Rocket.Chat server's (MIT data, data/LICENSE-joypixels-emoji-toolkit),
//!   plus each emoji's Unicode name in snake case: the server's emojibase
//!   names follow those (`folded_hands`, measured in a real reaction).

use std::collections::HashMap;
use std::sync::OnceLock;

pub type Table = HashMap<&'static str, &'static str>;

fn parse(tsv: &'static str) -> impl Iterator<Item = (&'static str, &'static str)> {
    tsv.lines().filter_map(|l| l.split_once('\t'))
}

/// iamcal names (Mattermost).
pub fn iamcal() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| parse(include_str!("../data/iamcal.tsv")).collect())
}

/// joypixels names, with iamcal names as fallback for what other clients
/// may send (Rocket.Chat: only 1,338 of the 4,278 joypixels names are
/// iamcal names too).
pub fn rocketchat() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t: Table = iamcal().clone();
        t.extend(parse(include_str!("../data/joypixels.tsv")));
        t
    })
}

/// A table as owned pairs.
pub fn pairs(table: &Table) -> Vec<(String, String)> {
    table
        .iter()
        .map(|(n, c)| ((*n).to_owned(), (*c).to_owned()))
        .collect()
}

/// Name for a code, from the iamcal table (Mattermost's names; the
/// alphabetically first when several share a code).
fn names_by_code() -> &'static HashMap<&'static str, &'static str> {
    static NAMES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut m: HashMap<&str, &str> = HashMap::new();
        for (name, code) in iamcal() {
            let e = m.entry(code).or_insert(name);
            if name < e {
                *e = name;
            }
        }
        m
    })
}

/// (name, code) of an emoji as sent by apps that send the character
/// itself (Telegram, Signal): the table's code, with or without the
/// variation selector ("❤" where the table has "❤️"). An emoji not in the
/// table is its own name.
pub fn name_and_code(emoji: &str) -> (String, String) {
    let code: Vec<String> = emoji.chars().map(|c| format!("{:x}", c as u32)).collect();
    let code = code.join("-");
    let bare = code.replace("-fe0f", "");
    let names = names_by_code();
    for c in [code.clone(), format!("{bare}-fe0f"), bare] {
        if let Some((code, name)) = names.get_key_value(c.as_str()) {
            return ((*name).to_owned(), (*code).to_owned());
        }
    }
    (emoji.to_owned(), code)
}

/// The emoji character(s) for a name from the table, else the name itself
/// (an emoji named after itself, see `name_and_code`).
pub fn emoji_for(name: &str) -> String {
    iamcal()
        .get(name)
        .and_then(|code| {
            code.split('-')
                .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
                .collect::<Option<String>>()
        })
        .unwrap_or_else(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables() {
        assert_eq!(iamcal().get("+1"), Some(&"1f44d"));
        assert_eq!(iamcal().get("right_facing_fist"), None);
        let rc = rocketchat();
        assert_eq!(rc.get("right_facing_fist"), Some(&"1f91c"), "joypixels");
        assert_eq!(
            rc.get("right-facing_fist"),
            Some(&"1f91c"),
            "iamcal fallback"
        );
        assert_eq!(rc.get("heart"), Some(&"2764-fe0f"));
        assert_eq!(rc.get("folded_hands"), Some(&"1f64f"), "Unicode name");
        assert_eq!(rc.get("pray"), Some(&"1f64f"));
        assert_eq!(rc.get("check_mark_button"), rc.get("white_check_mark"));
        assert!(rc.len() > iamcal().len());
    }
}
