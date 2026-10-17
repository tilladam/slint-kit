# emoji-shortcodes

Emoji shortcode tables and Unicode conversion for chat applications. This crate
has no dependencies and does not depend on Slint.

```toml
[dependencies]
emoji-shortcodes = "0.1.0"
```

```rust
assert_eq!(emoji_shortcodes::emoji_for("+1"), "👍");
assert_eq!(emoji_shortcodes::iamcal().get("+1"), Some(&"1f44d"));
assert_eq!(emoji_shortcodes::name_and_code("👍"), ("+1".into(), "1f44d".into()));
assert_eq!(emoji_shortcodes::rocketchat().get("folded_hands"), Some(&"1f64f"));
```

`iamcal()` provides Mattermost-style names. `rocketchat()` combines JoyPixels
names with iamcal fallbacks; JoyPixels wins collisions. Table values are
hexadecimal Unicode code points separated by hyphens. `emoji_for()` uses the
iamcal table and returns an unknown name unchanged. `pairs()` returns owned
name/code pairs.

Part of [slint-kit](https://github.com/tilladam/slint-kit).
[API documentation](https://docs.rs/emoji-shortcodes).

MIT licensed. The bundled datasets retain their upstream MIT notices in
`data/LICENSE-iamcal-emoji-data` and `data/LICENSE-joypixels-emoji-toolkit`.
No emoji artwork is included.
