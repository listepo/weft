//! Swift lexical rules shared by the generator and the importer: string literals, identifiers
//! and the names the generator derives from Weft ids, paths and actions.

/// Words that cannot stand bare as a declared name or after a `.` in Swift 6. Contextual
/// keywords (`get`, `some`, `async`…) are valid identifiers and are not listed.
const KEYWORDS: &[&str] = &[
    "Any",
    "Protocol",
    "Self",
    "Type",
    "as",
    "associatedtype",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "operator",
    "precedencegroup",
    "private",
    "protocol",
    "public",
    "repeat",
    "rethrows",
    "return",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];

pub fn is_keyword(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

/// `[A-Za-z_][A-Za-z0-9_]*` and not a keyword: usable without backticks.
pub fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    starts
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name != "_"
        && !is_keyword(name)
}

/// A member name: bare when it can be, otherwise a raw identifier in backticks (SE-0451). The
/// importer reads `isEmpty` after a binding as a truth test, so a data field of that name is
/// always written with backticks.
pub fn member(name: &str) -> String {
    if is_plain_identifier(name) && name != "isEmpty" {
        name.to_owned()
    } else {
        format!("`{name}`")
    }
}

/// The text of an identifier token without the backticks of an escaped or raw identifier.
#[cfg(feature = "import")]
pub fn unescape_identifier(text: &str) -> &str {
    text.strip_prefix('`')
        .and_then(|t| t.strip_suffix('`'))
        .unwrap_or(text)
}

/// A Swift string literal that reads back as exactly `s`: no interpolation, every character
/// that has meaning inside a literal escaped.
pub fn string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `load-error` → `LoadError`: the prefix of every type generated for a screen. Always followed
/// by a suffix (`Model`, `Screen`…), so it only has to start like an identifier.
pub fn type_prefix(id: &str) -> String {
    let mut out = String::new();
    for part in id
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
    {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out = format!("Weft{out}");
    }
    out
}

/// `auth.submit` → `authSubmit`. Action segments match `[a-z][A-Za-z0-9]*`, so the result is an
/// identifier; a keyword gets a trailing `_`.
pub fn action_case(action: &str) -> String {
    let mut out = String::new();
    for (i, part) in action.split('.').enumerate() {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.push(first);
            } else {
                out.extend(first.to_uppercase());
            }
            out.push_str(chars.as_str());
        }
    }
    if is_plain_identifier(&out) {
        out
    } else {
        format!("{out}_")
    }
}

/// `rating` → `RatingView`: the view the app writes for a kind of its own catalog (SPEC §9).
pub fn view_name(kind: &str) -> String {
    format!("{}View", type_prefix(kind))
}

/// `max-length` → `maxLength`: an argument label for a prop, slot or event name. Names follow
/// `[a-z][a-z0-9]*(-[a-z0-9]+)*` (SPEC §2), so only a keyword needs backticks.
pub fn label(name: &str) -> String {
    let mut out = String::new();
    for (i, part) in name.split('-').enumerate() {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            if i == 0 {
                out.push(first);
            } else {
                out.extend(first.to_uppercase());
            }
            out.push_str(chars.as_str());
        }
    }
    member(&out)
}

/// `press` → `onPress`: the closure argument a custom view takes for an event.
pub fn event_label(event: &str) -> String {
    label(&format!("on-{event}"))
}

/// A token path segment as a member name. Swift reserves the `$` prefix, so the DTCG `$root`
/// token is `_root`, and a token really named `_root` is written in backticks to stay apart
/// from it when the importer reads the path back.
pub fn token_member(segment: &str) -> String {
    match segment {
        "$root" => "_root".to_owned(),
        "_root" => "`_root`".to_owned(),
        _ => member(segment),
    }
}

/// A Swift number literal for a finite value: integers without a fraction.
pub fn number_literal(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.is_finite() {
        format!("{v}")
    } else {
        "0".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_literals_escape_everything_swift_reads_specially() {
        assert_eq!(string_literal("a\"b\\c\n\\(x)"), r#""a\"b\\c\n\\(x)""#);
        assert_eq!(string_literal("\u{1}"), r#""\u{1}""#);
    }

    #[test]
    fn names_follow_swift_conventions() {
        assert_eq!(type_prefix("load-error"), "LoadError");
        assert_eq!(type_prefix("a_b-c"), "ABC");
        assert_eq!(type_prefix("2 fa.weft"), "Weft2FaWeft");
        assert_eq!(action_case("auth.submit"), "authSubmit");
        assert_eq!(action_case("default"), "default_");
        assert_eq!(member("default"), "`default`");
        assert_eq!(member("isEmpty"), "`isEmpty`");
        assert_eq!(member("my-x"), "`my-x`");
        assert_eq!(member("email"), "email");
        assert_eq!(number_literal(16.0), "16");
        assert_eq!(number_literal(14.5), "14.5");
        assert_eq!(view_name("rating-bar"), "RatingBarView");
        assert_eq!(label("max-length"), "maxLength");
        assert_eq!(label("default"), "`default`");
        assert_eq!(event_label("press"), "onPress");
        assert_eq!(token_member("$root"), "_root");
        assert_eq!(token_member("_root"), "`_root`");
    }
}
