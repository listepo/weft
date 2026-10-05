//! JavaScript's readings of values, for generators whose output must match what the reference
//! renderer (`@weft/render-react`) computes at run time and what the former TypeScript generator
//! printed.

use std::cmp::Ordering;

use serde_json::Value as Json;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
use weft_core::{js_number, to_compact};

/// A value as a generator sees it at compile time: JSON, plus `undefined`. Arrays matter only for
/// their truthiness and length.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum V {
    Undef,
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(usize),
}

impl V {
    pub(crate) fn of(raw: Option<&Json>) -> V {
        match raw {
            None | Some(Json::Object(_)) => V::Undef,
            Some(Json::Null) => V::Null,
            Some(Json::Bool(b)) => V::Bool(*b),
            Some(Json::Number(n)) => n.as_f64().map_or(V::Undef, V::Num),
            Some(Json::String(s)) => V::Str(s.clone()),
            Some(Json::Array(a)) => V::Arr(a.len()),
        }
    }

    /// The renderer's `text`: strings as they are, finite numbers and booleans printed, the rest
    /// empty.
    pub(crate) fn text(&self) -> String {
        weft_core::text(self.json().as_ref())
    }

    /// The renderer's `flag`: JavaScript truthiness, except that the string `"false"` is false.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            // JSON has no infinities, so the shared reading never sees one.
            V::Num(n) if n.is_infinite() => true,
            _ => weft_core::truthy(self.json().as_ref()),
        }
    }

    /// The value as JSON, where it is one; NaN and the infinities are not. The length of an
    /// array is not part of either reading, so an empty array stands in for any.
    fn json(&self) -> Option<Json> {
        match self {
            V::Undef => None,
            V::Null => Some(Json::Null),
            V::Bool(b) => Some(Json::Bool(*b)),
            V::Num(n) => serde_json::Number::from_f64(*n).map(Json::Number),
            V::Str(s) => Some(Json::String(s.clone())),
            V::Arr(_) => Some(Json::Array(vec![])),
        }
    }

    /// `String(v)` for the values a generator prints into markup.
    pub(crate) fn string(&self) -> String {
        match self {
            V::Undef => "undefined".to_owned(),
            V::Null => "null".to_owned(),
            V::Arr(_) => String::new(),
            other => other.text(),
        }
    }

    /// The value as a JavaScript expression. A literal array is not a SPEC value; only its
    /// truthiness can matter.
    pub(crate) fn literal(&self) -> String {
        match self {
            V::Str(s) => quote(s),
            V::Num(n) if n.is_nan() => "NaN".to_owned(),
            V::Num(n) if n.is_infinite() => {
                if *n > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
            }
            V::Num(n) => js_number(*n),
            V::Bool(b) => b.to_string(),
            V::Null => "null".to_owned(),
            V::Arr(_) => "[]".to_owned(),
            V::Undef => "undefined".to_owned(),
        }
    }
}

/// `JSON.stringify` of a string: a JavaScript string literal.
pub(crate) fn quote(s: &str) -> String {
    to_compact(s)
}

pub(crate) use weft_core::js_round;

/// `Number.isInteger`.
pub(crate) fn is_integer(x: f64) -> bool {
    x.is_finite() && x.fract() == 0.0
}

/// `a < b` for strings as `Array.prototype.sort` compares them: by UTF-16 code units, which
/// orders characters past U+FFFF before U+E000–U+FFFF unlike Rust's code-point order.
pub(crate) fn compare_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Characters that mean nothing to the JSX lexer inside text or a quoted attribute: letters,
/// digits and a few punctuation marks (`/^[\p{L}\p{N} .,:;!?'()/#%+=@_*~^|$-]*$/u`).
pub(crate) fn is_inert(s: &str) -> bool {
    s.chars().all(|c| {
        " .,:;!?'()/#%+=@_*~^|$-".contains(c)
            || matches!(
                c.general_category_group(),
                GeneralCategoryGroup::Letter | GeneralCategoryGroup::Number
            )
    })
}

/// JSX text keeps a run as written only when it neither starts nor ends with a space and holds no
/// two spaces in a row, which JSX would otherwise collapse or trim.
pub(crate) fn is_inert_text(s: &str) -> bool {
    !s.is_empty() && !s.starts_with(' ') && !s.ends_with(' ') && !s.contains("  ")
}

/// Names a plain `?.` path must not reach: what `Object`, `Array`, `String`, `Number` and
/// `Boolean` prototypes hold (Node 26), where the renderer reads own properties only. A name
/// added in a later engine only costs a `_get` call, so the list errs on the long side.
pub(crate) const INHERITED: &[&str] = &[
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "__proto__",
    "anchor",
    "at",
    "big",
    "blink",
    "bold",
    "charAt",
    "charCodeAt",
    "codePointAt",
    "concat",
    "constructor",
    "copyWithin",
    "endsWith",
    "entries",
    "every",
    "fill",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "fixed",
    "flat",
    "flatMap",
    "fontcolor",
    "fontsize",
    "forEach",
    "hasOwnProperty",
    "includes",
    "indexOf",
    "isPrototypeOf",
    "isWellFormed",
    "italics",
    "join",
    "keys",
    "lastIndexOf",
    "length",
    "link",
    "localeCompare",
    "map",
    "match",
    "matchAll",
    "normalize",
    "padEnd",
    "padStart",
    "pop",
    "propertyIsEnumerable",
    "push",
    "reduce",
    "reduceRight",
    "repeat",
    "replace",
    "replaceAll",
    "reverse",
    "search",
    "shift",
    "slice",
    "small",
    "some",
    "sort",
    "splice",
    "split",
    "startsWith",
    "strike",
    "sub",
    "substr",
    "substring",
    "sup",
    "toExponential",
    "toFixed",
    "toLocaleLowerCase",
    "toLocaleString",
    "toLocaleUpperCase",
    "toLowerCase",
    "toPrecision",
    "toReversed",
    "toSorted",
    "toSpliced",
    "toString",
    "toUpperCase",
    "toWellFormed",
    "trim",
    "trimEnd",
    "trimLeft",
    "trimRight",
    "trimStart",
    "unshift",
    "valueOf",
    "values",
    "with",
];

/// `[A-Za-z_][A-Za-z0-9_]*`: a path segment `?.` can name.
pub(crate) fn is_plain_segment(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_follows_math_round() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_round(9.4), 9.0);
    }

    #[test]
    fn inert_runs_hold_letters_digits_and_safe_punctuation() {
        assert!(is_inert("Crème brûlée 日本 42%"));
        assert!(!is_inert("a{b}"));
        assert!(!is_inert("a<b"));
        assert!(!is_inert("\"q\""));
        assert!(!is_inert("a&b"));
        assert!(is_inert_text("a b"));
        assert!(!is_inert_text(" a"));
        assert!(!is_inert_text("a  b"));
    }

    #[test]
    fn strings_sort_by_utf16_units() {
        assert_eq!(compare_utf16("\u{10000}", "\u{FFFF}"), Ordering::Less);
        assert_eq!(compare_utf16("a", "b"), Ordering::Less);
    }

    #[test]
    fn inherited_names_are_sorted_and_plain() {
        assert!(INHERITED.windows(2).all(|w| w[0] < w[1]));
        assert!(INHERITED.iter().all(|s| is_plain_segment(s)));
    }
}
