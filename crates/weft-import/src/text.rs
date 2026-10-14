//! Text as the TypeScript importers treat it. Whitespace is ECMAScript's (`\s` and `trim`), not
//! Rust's: the two disagree on U+0085 and U+FEFF, and imported names must not depend on the host.

use unicode_normalization::UnicodeNormalization;
use weft_core::{embedded_reference, is_non_xml_char};
pub use weft_core::{is_js_space, js_number_from, js_trim, squash};

use crate::loss::{LossKind, Losses};

/// The first `units` UTF-16 code units, as `String.prototype.slice(0, units)` counts; a pair cut in
/// half is left out, since a lone surrogate cannot be text.
pub fn js_prefix(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > units {
            return &s[..i];
        }
    }
    s
}

/// Length as JavaScript counts it, in UTF-16 code units.
pub fn js_length(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Drops the characters markup cannot carry (W221).
pub fn clean(s: &str) -> String {
    s.chars().filter(|&c| !is_non_xml_char(c)).collect()
}

/// A readable id fragment: lower-case ASCII words joined by hyphens, at most 32 characters.
pub fn slug(text: &str) -> String {
    let lower: String = text
        .nfkd()
        .filter(|c| !('\u{300}'..='\u{36F}').contains(c))
        .collect::<String>()
        .to_lowercase();
    let mut out = String::new();
    let mut gap = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c);
            if out.len() >= 32 {
                break;
            }
        } else {
            gap = true;
        }
    }
    // A hyphen that would end the slug at the cut is dropped with the rest.
    out.truncate(32);
    out.trim_end_matches('-').to_owned()
}

/// A literal must not contain a reference after its first character (SPEC §2.1, W213); such text
/// is real content, so the brace is replaced by a look-alike instead of dropping it.
pub fn literal(losses: &mut Losses, path: &str, s: &str) -> String {
    let out = clean(s);
    if embedded_reference(&out).is_none_or(|i| i == 0) {
        return out;
    }
    losses.push(
        LossKind::Text,
        path,
        "text that reads as a binding or token reference had its brace replaced",
    );
    let mut chars = out.chars();
    let mut result = String::with_capacity(out.len() + 2);
    if let Some(first) = chars.next() {
        result.push(first);
    }
    let rest = chars.as_str();
    for (i, c) in rest.char_indices() {
        let after = &rest[i + c.len_utf8()..];
        if c == '{'
            && (after.starts_with('$') || after.starts_with("!$") || after.starts_with("token."))
        {
            result.push('｛');
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_count_utf16_units() {
        assert_eq!(js_prefix("abc", 2), "ab");
        assert_eq!(js_prefix("a😀b", 2), "a");
        assert_eq!(js_prefix("a😀b", 3), "a😀");
        assert_eq!(js_length("a😀b"), 4);
    }

    #[test]
    fn slugs_are_ascii_words() {
        assert_eq!(slug("Save changes!"), "save-changes");
        assert_eq!(slug("  Crème brûlée "), "creme-brulee");
        assert_eq!(slug("ﬁle"), "file");
        assert_eq!(slug("--"), "");
        assert_eq!(slug("日本"), "");
        assert_eq!(slug(&"ab ".repeat(20)), "ab-ab-ab-ab-ab-ab-ab-ab-ab-ab-ab");
        assert_eq!(
            slug("abcdefghijklmnopqrstuvwxyzabcde fgh"),
            "abcdefghijklmnopqrstuvwxyzabcde"
        );
    }

    #[test]
    fn literals_never_form_references() {
        let mut losses = Losses::default();
        assert_eq!(
            literal(&mut losses, "/p", "a {$.x} and {token.y}"),
            "a ｛$.x} and ｛token.y}"
        );
        assert_eq!(losses.0.len(), 1);
        // A reference at the very start is the whole value's concern (W116/W213), as in the
        // TypeScript importer.
        assert_eq!(literal(&mut losses, "/p", "{$.x} {$.y}"), "{$.x} {$.y}");
        assert_eq!(literal(&mut losses, "/p", "a\u{0}b {!$.x}"), "ab ｛!$.x}");
        assert_eq!(literal(&mut losses, "/p", "{plain}"), "{plain}");
        assert_eq!(losses.0.len(), 2);
    }
}
