//! Inline-text helpers for message markdown: bare-URL linking, link
//! extraction, `:shortcode:` emoji and custom-emoji detection. All of them
//! touch only literal text — never code (inline or fenced), links or images.
//!
//! Extracted from yapper (`crates/yapper-ui/src/blocks.rs`).

use emoji_shortcodes::Table;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Link targets in a message, in order and without repeats: markdown
/// links and autolinks, plus bare `http(s)://` URLs in text, which Zulip
/// links on its own. Text inside code is not searched.
pub fn links(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |u: &str| {
        if !u.is_empty() && !out.iter().any(|o| o == u) {
            out.push(u.to_owned());
        }
    };
    let linked = linkify(src);
    for e in Parser::new_ext(
        &linked,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES,
    ) {
        if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = e {
            push(&dest_url);
        }
    }
    out
}

/// CommonMark does not recognize bare web URLs. Add autolink delimiters only
/// to literal text, leaving existing links, inline code and code blocks alone.
pub fn linkify(src: &str) -> String {
    let mut protected = 0;
    let mut ranges = Vec::new();
    let mut events = Parser::new_ext(src, Options::ENABLE_STRIKETHROUGH)
        .into_offset_iter()
        .peekable();
    while let Some((event, mut range)) = events.next() {
        match event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                protected += 1
            }
            Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => protected -= 1,
            Event::Text(text) if protected == 0 && src[range.clone()] == *text => {
                // The parser may split literal underscores into separate text
                // events, even when they belong to a single URL.
                while let Some((Event::Text(next), next_range)) = events.peek() {
                    if range.end != next_range.start || src[next_range.clone()] != **next {
                        break;
                    }
                    range.end = next_range.end;
                    events.next();
                }
                let text = &src[range.clone()];
                let mut offset = 0;
                while let Some(start) = text[offset..]
                    .find("http://")
                    .into_iter()
                    .chain(text[offset..].find("https://"))
                    .min()
                    .map(|i| offset + i)
                {
                    let tail = &text[start..];
                    let len = tail
                        .find(|c: char| c.is_whitespace() || "<>\"'".contains(c))
                        .unwrap_or(tail.len());
                    let mut url = tail[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
                    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
                        while url.ends_with(close)
                            && url.matches(close).count() > url.matches(open).count()
                        {
                            url = &url[..url.len() - 1];
                        }
                    }
                    ranges.push(range.start + start..range.start + start + url.len());
                    offset = start + len;
                }
            }
            _ => {}
        }
    }
    let mut result = String::with_capacity(src.len() + ranges.len() * 2);
    let mut copied = 0;
    for range in ranges {
        result.push_str(&src[copied..range.start]);
        result.push('<');
        result.push_str(&src[range.clone()]);
        result.push('>');
        copied = range.end;
    }
    result.push_str(&src[copied..]);
    result
}

/// Emoji shortcodes in message text (`:thumbsup:`, as Rocket.Chat and
/// Mattermost send them) as emoji, for names in the shared emoji tables.
/// Only literal text is touched: not code (inline or fenced), links or
/// images; unknown names, times like `10:30:45` and URLs stay as they are.
/// Applied when a message is rendered; the stored body is unchanged.
/// `table` is one of [`emoji_shortcodes::iamcal`] or
/// [`emoji_shortcodes::rocketchat`] (or your own).
pub fn emoji_shortcodes(src: &str, table: &Table) -> String {
    let mut result = String::with_capacity(src.len());
    let mut copied = 0;
    for range in literal_ranges(src) {
        if let Some(new) = replace_shortcodes(&src[range.clone()], table) {
            result.push_str(&src[copied..range.start]);
            result.push_str(&new);
            copied = range.end;
        }
    }
    result.push_str(&src[copied..]);
    result
}

/// The `:name:` shortcodes in `text` as (byte range of the whole token,
/// name): a name of letters, digits, `_`, `+` and `-`, not glued to a word,
/// path or number on either side (so `10:30:45` and `a/:b:` are not).
fn scan_shortcodes(text: &str) -> Vec<(std::ops::Range<usize>, &str)> {
    if !text.contains(':') {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-');
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].1 == ':' {
            let prev_ok = i == 0 || {
                let p = chars[i - 1].1;
                !p.is_alphanumeric() && !"/.=&%#@".contains(p)
            };
            let mut j = i + 1;
            while j < chars.len() && name_char(chars[j].1) {
                j += 1;
            }
            let closed = j < chars.len() && chars[j].1 == ':' && j > i + 1;
            let next_ok = j + 1 >= chars.len() || !chars[j + 1].1.is_alphanumeric();
            if prev_ok && closed && next_ok {
                let end = chars[j].0 + 1;
                found.push((chars[i].0..end, &text[chars[i + 1].0..chars[j].0]));
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    found
}

/// `text` with its known `:name:` shortcodes replaced; `None` if none was.
fn replace_shortcodes(text: &str, table: &Table) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    for (range, name) in scan_shortcodes(text) {
        let emoji = table.get(name).and_then(|code| {
            code.split('-')
                .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
                .collect::<Option<String>>()
        });
        if let Some(emoji) = emoji {
            out.push_str(&text[copied..range.start]);
            out.push_str(&emoji);
            copied = range.end;
        }
    }
    if copied == 0 {
        return None;
    }
    out.push_str(&text[copied..]);
    Some(out)
}

/// The literal-text ranges of `src`: not code, links or images.
pub fn literal_ranges(src: &str) -> Vec<std::ops::Range<usize>> {
    let mut protected = 0;
    let mut ranges = Vec::new();
    let mut events = Parser::new_ext(src, Options::ENABLE_STRIKETHROUGH)
        .into_offset_iter()
        .peekable();
    while let Some((event, mut range)) = events.next() {
        match event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                protected += 1
            }
            Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => protected -= 1,
            Event::Text(text) if protected == 0 && src[range.clone()] == *text => {
                // The parser splits text at underscores and the like.
                while let Some((Event::Text(next), next_range)) = events.peek() {
                    if range.end != next_range.start || src[next_range.clone()] != **next {
                        break;
                    }
                    range.end = next_range.end;
                    events.next();
                }
                ranges.push(range);
            }
            _ => {}
        }
    }
    ranges
}

/// The custom emoji a message uses: the names of its literal `:name:`
/// tokens for which `is_custom` holds, in order of use (repeats kept, at
/// most `MAX_EMOJI`), and whether the message is nothing but such emoji.
pub fn custom_emoji(src: &str, is_custom: &dyn Fn(&str) -> bool) -> (Vec<String>, bool) {
    const MAX_EMOJI: usize = 16;
    // Cheap first pass over the raw text: parse only when a candidate is
    // a custom emoji.
    if !scan_shortcodes(src).iter().any(|(_, n)| is_custom(n)) {
        return (Vec::new(), false);
    }
    let mut names = Vec::new();
    let mut rest = String::with_capacity(src.len());
    let mut copied = 0;
    for range in literal_ranges(src) {
        for (tok, name) in scan_shortcodes(&src[range.clone()]) {
            if is_custom(name) {
                names.push(name.to_owned());
                rest.push_str(&src[copied..range.start + tok.start]);
                copied = range.start + tok.end;
            }
        }
    }
    rest.push_str(&src[copied..]);
    // Emoji only: nothing but whitespace is left (code, links and markup
    // count as content).
    let only = !names.is_empty() && rest.trim().is_empty();
    names.truncate(MAX_EMOJI);
    (names, only)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// yapper's choice: the Rocket.Chat table (joypixels plus iamcal).
    fn sc(src: &str) -> String {
        emoji_shortcodes(src, emoji_shortcodes::rocketchat())
    }

    #[test]
    fn custom_emoji_are_found_in_literal_text_in_order() {
        let custom = |n: &str| matches!(n, "party" | "wave_2");
        let find = |s: &str| custom_emoji(s, &custom);
        assert_eq!(
            find("hi :party: and :smile: :wave_2: :party:"),
            (vec!["party".into(), "wave_2".into(), "party".into()], false)
        );
        assert_eq!(find("no emoji here, at 10:30:45"), (vec![], false));
        assert_eq!(find(":smile: only standard"), (vec![], false));
        // Emoji only: tokens and whitespace, nothing else.
        assert_eq!(find(":party:"), (vec!["party".into()], true));
        assert_eq!(
            find(" :party::wave_2:\n:party: "),
            (vec!["party".into(), "wave_2".into(), "party".into()], true)
        );
        assert!(
            !find(":party: :smile:").1,
            "a standard shortcode is content"
        );
        assert!(!find("**:party:**").1, "markup is content");
        // Code, links and URLs do not count.
        assert_eq!(find("`:party:`"), (vec![], false));
        assert_eq!(find("[:party:](http://x.org/:party:)"), (vec![], false));
        assert_eq!(find("http://x.org/a:party:"), (vec![], false));
        assert_eq!(find("```\n:party:\n```"), (vec![], false));
        assert_eq!(
            find("`code :party:` then :party:"),
            (vec!["party".into()], false)
        );
        // At most 16.
        let many = ":party:".repeat(20);
        assert_eq!(find(&many).0.len(), 16);
    }

    #[test]
    fn rocketchat_emojibase_shortcodes_render_in_message_text() {
        assert_eq!(
            sc(":check_mark_button: First item\n:check_mark_button: Second item"),
            "✅ First item\n✅ Second item"
        );
        assert_eq!(
            sc("`:check_mark_button:` and :check_mark_button:"),
            "`:check_mark_button:` and ✅"
        );
    }

    #[test]
    fn shortcodes_become_emoji_only_in_literal_text() {
        assert_eq!(sc(":thumbsup:"), "👍\u{fe0f}", "table code 1f44d-fe0f");
        assert_eq!(sc("ok :+1: and :heart:!"), "ok 👍\u{fe0f} and ❤\u{fe0f}!");
        assert_eq!(sc(":smile::wink:"), "😄😉", "adjacent");
        assert_eq!(sc("**bold :fire:**"), "**bold 🔥**");
        // Unknown names, times, URLs, lone colons stay.
        assert_eq!(sc("a :no_such_emoji_x: b"), "a :no_such_emoji_x: b");
        assert_eq!(sc("at 10:30:45 sharp"), "at 10:30:45 sharp");
        assert_eq!(sc("http://x.org/a:smile:"), "http://x.org/a:smile:");
        assert_eq!(sc("note: this: that"), "note: this: that");
        assert_eq!(sc("::"), "::");
        // Code, links and images are left alone.
        assert_eq!(sc("`:smile:` and :smile:"), "`:smile:` and 😄");
        assert_eq!(sc("``a :smile: b``"), "``a :smile: b``");
        assert_eq!(
            sc("[:smile:](http://x.org/:smile:)"),
            "[:smile:](http://x.org/:smile:)"
        );
        assert_eq!(sc("```\n:smile:\n```"), "```\n:smile:\n```");
        // Names with underscores (split by the parser) and non-ASCII text.
        assert_eq!(sc("größe :point_up_2: ü"), "größe 👆\u{fe0f} ü");
    }

    #[test]
    fn links_in_order_without_code_or_repeats() {
        let src = "See [docs](https://a.org/x) and https://b.org/y.\n\n`https://code.org` \
                   <https://c.org> [again](https://a.org/x) ![img](/user_uploads/1/p.png)";
        assert_eq!(
            links(src),
            [
                "https://a.org/x",
                "https://b.org/y",
                "https://c.org",
                "/user_uploads/1/p.png"
            ]
        );
    }

    #[test]
    fn bare_urls_become_links_without_changing_code_or_existing_links() {
        let src = "See https://example.org/a and (https://example.org/Foo_(bar)).";
        assert_eq!(
            linkify(src),
            "See <https://example.org/a> and (<https://example.org/Foo_(bar)>)."
        );
        assert_eq!(
            links(src),
            ["https://example.org/a", "https://example.org/Foo_(bar)"]
        );
        let src = "[https://label.org](https://target.org) <https://auto.org> `https://code.org`\n\n```\nhttps://code.org\n```";
        assert_eq!(linkify(src), src);
        assert_eq!(links(src), ["https://target.org", "https://auto.org"]);
        assert_eq!(
            linkify("é https://example.org/ä?a=1&b=2!"),
            "é <https://example.org/ä?a=1&b=2>!"
        );
    }
}
