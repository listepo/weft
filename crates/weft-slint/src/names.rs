//! How Weft names and strings become Slint source: identifiers, component names and string
//! literals. Kept apart because these are the trust boundary of the generator: document text
//! reaches the output only through `string`, and ids only after `identifier` accepted them.

/// Ids Slint gives a meaning of their own inside a component, and the globals and enums the
/// generated code names: an element id would shadow them.
const RESERVED: &[&str] = &[
    "root",
    "self",
    "parent",
    "true",
    "false",
    "Palette",
    "InputType",
    "LayoutAlignment",
];

/// The key two Slint identifiers clash on: Slint reads `-` and `_` in an identifier as the same
/// character.
pub fn clash_key(name: &str) -> String {
    name.replace('_', "-")
}

/// A Weft id or data path segment as a Slint identifier, when Slint takes it as one.
pub fn identifier(name: &str) -> Option<&str> {
    let mut chars = name.chars();
    let starts = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let reserved = RESERVED.contains(&clash_key(name).as_str());
    (starts && rest && !reserved).then_some(name)
}

/// `login-form` → `LoginForm`: the component name a screen id gives, with `Screen` appended by
/// the caller. A name that would start with a digit gets a `Weft` prefix.
pub fn type_name(id: &str) -> String {
    let mut out = String::new();
    for part in id.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert_str(0, "Weft");
    }
    out
}

/// A Slint string literal. Every backslash is doubled, so no `\{` interpolation can form, and
/// control and line-separator characters are written as `\u{…}`, so a literal never spans lines.
pub fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_cannot_interpolate_or_break_lines() {
        assert_eq!(string(r#"a\{b}"c"#), r#""a\\{b}\"c""#);
        assert_eq!(string("x\ny\u{1}\u{2028}"), r#""x\ny\u{1}\u{2028}""#);
    }

    #[test]
    fn reserved_and_malformed_ids_are_refused() {
        assert_eq!(identifier("email"), Some("email"));
        assert_eq!(identifier("sub_mit-2"), Some("sub_mit-2"));
        assert_eq!(identifier("root"), None);
        assert_eq!(identifier("pa_rent".replace('_', "").as_str()), None);
        assert_eq!(identifier("2fa"), None);
    }

    #[test]
    fn dashes_and_underscores_clash() {
        assert_eq!(clash_key("a_b"), clash_key("a-b"));
    }

    #[test]
    fn screen_ids_become_component_names() {
        assert_eq!(type_name("login"), "Login");
        assert_eq!(type_name("confirm-dialog"), "ConfirmDialog");
        assert_eq!(type_name("2fa"), "Weft2fa");
    }
}
