//! Grammars and fixed vocabulary of SPEC §2–§4 and §8, shared by the parser, serializer and
//! validator so that every layer agrees on what a valid name, id or reference is. The grammars
//! are small enough to match by hand, which keeps a regex engine out of the WebAssembly build.

use std::sync::LazyLock;

use crate::model::{PropDef, PropType};

/// Deeper trees are rejected so that recursive walkers cannot exhaust the stack on hostile input.
pub const MAX_DEPTH: usize = 256;

pub const SLOT: &str = "slot";
pub const EACH: &str = "each";

fn lower_alnum(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `[a-z][a-z0-9]*(-[a-z0-9]+)*`: element and attribute names (SPEC §2).
pub fn is_name(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.split('-')
            .all(|part| !part.is_empty() && part.chars().all(lower_alnum))
}

/// `x-<vendor>-<name>` (SPEC §8).
pub fn is_extension_name(s: &str) -> bool {
    s.strip_prefix("x-").is_some_and(|rest| {
        rest.split('-').count() >= 2
            && rest
                .split('-')
                .all(|part| !part.is_empty() && part.chars().all(lower_alnum))
    })
}

/// `[A-Za-z][A-Za-z0-9_-]*`
pub fn is_id(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `[a-z][A-Za-z0-9]*`
pub fn is_loop_variable(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// `[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*`
pub fn is_action(s: &str) -> bool {
    s.split('.').all(is_loop_variable)
}

fn is_identifier(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') && s.chars().all(word_char)
}

/// A binding path segment: an identifier or a non-negative integer without leading zeros.
fn is_segment(s: &str) -> bool {
    is_identifier(s)
        || s == "0"
        || (s.starts_with(|c: char| ('1'..='9').contains(&c))
            && s.chars().all(|c| c.is_ascii_digit()))
}

/// `$` (`.` segment)+ for the data model, or `$` identifier (`.` segment)* for a loop variable.
pub fn is_binding(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('$') else {
        return false;
    };
    match rest.strip_prefix('.') {
        Some(path) => path.split('.').all(is_segment),
        None => {
            let mut parts = rest.split('.');
            parts.next().is_some_and(is_identifier) && parts.all(is_segment)
        }
    }
}

/// Segments of `[A-Za-z0-9_-]+` joined by dots.
pub fn is_token(s: &str) -> bool {
    s.split('.').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    })
}

fn is_natural(s: &str) -> bool {
    s == "0"
        || (s.starts_with(|c: char| ('1'..='9').contains(&c))
            && s.chars().all(|c| c.is_ascii_digit()))
}

/// `(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)`, returning major and minor.
pub fn version(s: &str) -> Option<(&str, &str)> {
    let (major, minor) = s.split_once('.')?;
    (is_natural(major) && is_natural(minor)).then_some((major, minor))
}

/// The JSON number grammar.
pub fn is_json_number(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let (int, frac) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    let digits = |d: &str| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit());
    is_natural(int)
        && frac.is_none_or(digits)
        && exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)))
}

/// Byte index of the first `{$`, `{!$` or `{token.`: a reference written inside a value.
pub fn embedded_reference(s: &str) -> Option<usize> {
    s.match_indices('{').map(|(i, _)| i).find(|&i| {
        let rest = &s[i + 1..];
        rest.starts_with('$') || rest.starts_with("!$") || rest.starts_with("token.")
    })
}

/// Characters XML 1.0 cannot carry at all, not even as character references.
pub fn is_non_xml_char(c: char) -> bool {
    !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

pub fn has_non_xml_char(s: &str) -> bool {
    s.chars().any(is_non_xml_char)
}

/// Universal attributes of SPEC §2.2 other than `id` and `on-*`; `state` values come from the
/// component.
static UNIVERSAL_PROPS: LazyLock<[(&str, PropDef); 8]> = LazyLock::new(|| {
    let mut role = PropDef::new("ARIA role of an extension element.", PropType::String);
    role.bindable = Some(false);
    // Literal only: a renderer writes the matrix once, and a design tool draws one fixed projection.
    let angle = |description: &str| {
        let mut def = PropDef::new(description, PropType::Number);
        def.min = Some(-360.0);
        def.max = Some(360.0);
        def.bindable = Some(false);
        def
    };
    let mut perspective = PropDef::new("Viewer distance of a 3D tilt, in px.", PropType::Number);
    perspective.min = Some(1.0);
    perspective.bindable = Some(false);
    [
        ("label", PropDef::new("Accessible name.", PropType::String)),
        (
            "hidden",
            PropDef::new("Not rendered and not exposed.", PropType::Boolean),
        ),
        (
            "state",
            PropDef::new("One of the component's states.", PropType::Enum),
        ),
        ("role", role),
        ("rotate-x", angle("Rotation about the horizontal axis, in degrees.")),
        ("rotate-y", angle("Rotation about the vertical axis, in degrees.")),
        ("rotate-z", angle("Rotation in the screen plane, in degrees.")),
        ("perspective", perspective),
    ]
});

pub fn universal_prop(name: &str) -> Option<&'static PropDef> {
    UNIVERSAL_PROPS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, def)| def)
}

/// Non-abstract roles of WAI-ARIA 1.2, https://www.w3.org/TR/wai-aria-1.2/#role_definitions
pub const ARIA_ROLES: &[&str] = &[
    "alert",
    "alertdialog",
    "application",
    "article",
    "banner",
    "blockquote",
    "button",
    "caption",
    "cell",
    "checkbox",
    "code",
    "columnheader",
    "combobox",
    "complementary",
    "contentinfo",
    "definition",
    "deletion",
    "dialog",
    "document",
    "emphasis",
    "feed",
    "figure",
    "form",
    "generic",
    "grid",
    "gridcell",
    "group",
    "heading",
    "img",
    "insertion",
    "link",
    "list",
    "listbox",
    "listitem",
    "log",
    "main",
    "marquee",
    "math",
    "menu",
    "menubar",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "meter",
    "navigation",
    "none",
    "note",
    "option",
    "paragraph",
    "presentation",
    "progressbar",
    "radio",
    "radiogroup",
    "region",
    "row",
    "rowgroup",
    "rowheader",
    "scrollbar",
    "search",
    "searchbox",
    "separator",
    "slider",
    "spinbutton",
    "status",
    "strong",
    "subscript",
    "superscript",
    "switch",
    "tab",
    "table",
    "tablist",
    "tabpanel",
    "term",
    "textbox",
    "time",
    "timer",
    "toolbar",
    "tooltip",
    "tree",
    "treegrid",
    "treeitem",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_lowercase_hyphen_grammar() {
        for ok in ["a", "radio-group", "x-acme-map", "h1", "a-1"] {
            assert!(is_name(ok), "{ok}");
        }
        for bad in ["", "A", "1a", "a-", "-a", "a--b", "a_b", "aB"] {
            assert!(!is_name(bad), "{bad}");
        }
    }

    #[test]
    fn extensions_need_a_vendor_and_a_name() {
        assert!(is_extension_name("x-acme-map"));
        assert!(is_extension_name("x-acme-big-map"));
        assert!(!is_extension_name("x-map"));
        assert!(!is_extension_name("x--map"));
    }

    #[test]
    fn binding_paths_start_at_the_model_or_a_loop_variable() {
        for ok in ["$.a", "$.a.b.0", "$.a.10", "$item", "$item.title", "$_x.y"] {
            assert!(is_binding(ok), "{ok}");
        }
        for bad in ["$", "$.", "$..a", "$.a.01", "$1", "a", "$.a-b", "$item."] {
            assert!(!is_binding(bad), "{bad}");
        }
    }

    #[test]
    fn json_numbers_follow_the_json_grammar() {
        for ok in ["0", "-0", "1.5", "2e10", "1E+3", "-12.0e-4"] {
            assert!(is_json_number(ok), "{ok}");
        }
        for bad in ["", "01", "1.", ".5", "+1", "1e", "0x1", "NaN", "Infinity"] {
            assert!(!is_json_number(bad), "{bad}");
        }
    }

    #[test]
    fn versions_are_major_dot_minor() {
        assert_eq!(version("0.1"), Some(("0", "1")));
        assert_eq!(version("10.20"), Some(("10", "20")));
        assert_eq!(version("01.1"), None);
        assert_eq!(version("1"), None);
        assert_eq!(version("1.2.3"), None);
    }

    #[test]
    fn embedded_references_are_found_at_their_first_brace() {
        assert_eq!(embedded_reference("Hello {$.name}"), Some(6));
        assert_eq!(embedded_reference("{$.a}"), Some(0));
        assert_eq!(embedded_reference("a {b} {token.x}"), Some(6));
        assert_eq!(embedded_reference("plain"), None);
    }

    #[test]
    fn xml_excludes_controls_and_non_characters() {
        assert!(is_non_xml_char('\u{1}'));
        assert!(is_non_xml_char('\u{FFFE}'));
        assert!(!is_non_xml_char('\t'));
        assert!(!is_non_xml_char('\u{1F600}'));
    }
}
