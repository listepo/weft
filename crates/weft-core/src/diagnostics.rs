//! The diagnostic code registry of SPEC §6.1 and the helpers every layer uses to build repair
//! hints. Codes are a public API: a published code keeps its meaning forever, so new checks get
//! new codes instead of reusing old ones.

use serde::Serialize;

use crate::json::{quote_str, to_compact};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Lenient,
    Strict,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    /// Counts UTF-16 code units, as SPEC §2 requires.
    pub column: u32,
}

/// `Mode` severity: a warning in lenient mode and an error in strict mode (SPEC §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Registered {
    Error,
    Warning,
    Mode,
}

macro_rules! codes {
    ($($code:ident => $severity:ident, $summary:literal;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Code { $($code),* }

        impl Code {
            pub const ALL: &[Code] = &[$(Code::$code),*];

            pub fn as_str(self) -> &'static str {
                match self { $(Code::$code => stringify!($code)),* }
            }

            pub fn summary(self) -> &'static str {
                match self { $(Code::$code => $summary),* }
            }

            fn registered(self) -> Registered {
                match self { $(Code::$code => Registered::$severity),* }
            }
        }
    };
}

codes! {
    W101 => Error, "Malformed markup.";
    W102 => Error, "XML declaration or processing instruction.";
    W103 => Error, "DOCTYPE or other markup declaration.";
    W104 => Error, "CDATA section.";
    W105 => Error, "Element or attribute name breaks the name grammar.";
    W106 => Error, "Attribute value is not double-quoted.";
    W107 => Error, "Attribute without a value.";
    W108 => Error, "Duplicate attribute.";
    W109 => Error, "Closing tag does not match the open element.";
    W110 => Error, "Element, tag or attribute value is never closed.";
    W111 => Error, "Closing tag without an open element.";
    W112 => Error, "Unknown or malformed entity or character reference.";
    W113 => Error, "Character not allowed at this place.";
    W114 => Error, "Content outside the single root element.";
    W115 => Error, "Malformed comment.";
    W116 => Error, "Value starts with `{` but is not a reference.";
    W117 => Error, "Nesting deeper than the limit.";
    W118 => Error, "Misplaced or malformed `<slot>`.";
    W119 => Error, "Slot name used twice under one parent.";
    W120 => Error, "Misplaced or malformed `<context>`.";
    W121 => Error, "Misplaced or malformed `<entry>`.";
    W200 => Error, "Document does not have the canonical JSON shape.";
    W201 => Error, "Root element is not `screen`.";
    W202 => Error, "Element without `id`.";
    W203 => Error, "Value is not one of the allowed values.";
    W204 => Error, "Value has the wrong type.";
    W205 => Error, "Required attribute is missing.";
    W206 => Error, "Event not declared by the component.";
    W207 => Error, "Slot not declared by the component.";
    W208 => Error, "Required slot is missing.";
    W209 => Error, "`role` on a catalog component.";
    W210 => Error, "Extension element without `role`.";
    W211 => Error, "Not a WAI-ARIA role.";
    W212 => Error, "Id breaks the id grammar.";
    W213 => Error, "Text and a reference mixed in one value.";
    W214 => Error, "Binding path breaks the binding grammar.";
    W215 => Error, "Token path breaks the token grammar.";
    W216 => Error, "Action name breaks the action grammar.";
    W217 => Error, "Binding on a literal-only attribute.";
    W218 => Error, "Negated binding where it cannot apply.";
    W219 => Error, "`weft` version is not `major.minor`.";
    W220 => Error, "Extension name lacks the `x-<vendor>-` prefix.";
    W221 => Error, "String contains characters markup cannot carry.";
    W222 => Error, "Malformed `<each>`.";
    W223 => Error, "Kind, attribute, event or slot name is invalid or reserved.";
    W224 => Error, "Number outside the declared range or not whole.";
    W227 => Error, "Context entry status missing or not allowed for its kind.";
    W228 => Mode, "Context over a limit.";
    W229 => Error, "Context entry text or author name not allowed.";
    W230 => Error, "`version` is not a literal MAJOR.MINOR.PATCH.";
    W301 => Error, "Duplicate id.";
    W302 => Error, "Child kind not allowed here.";
    W303 => Error, "Parent kind not allowed for this component.";
    W304 => Error, "Content breaks the content model.";
    W305 => Error, "Loop variable not in scope.";
    W306 => Error, "Unknown design token.";
    W307 => Error, "Design token has the wrong type.";
    W308 => Error, "Unknown action.";
    W309 => Error, "Id reference points at no suitable element.";
    W310 => Error, "Text given both as content and as `text`.";
    W311 => Error, "Loop variable shadows an outer one.";
    W312 => Error, "`screen` below the root.";
    W313 => Error, "Submit button outside a `form`.";
    W314 => Error, "`<each>` without an element to repeat.";
    W315 => Error, "Binding path not declared in the data schema.";
    W316 => Error, "Bound data has a type the attribute does not take.";
    W317 => Error, "Asset path of a `model` is not an allowed path.";
    W318 => Error, "`grow` on an element whose parent is not a `stack`.";
    W401 => Mode, "Unknown element.";
    W402 => Mode, "Unknown attribute.";
    W403 => Mode, "Newer minor version of the format.";
    W404 => Error, "Unsupported major version of the format.";
    W501 => Error, "Patch list or patch has the wrong shape.";
    W502 => Error, "Patch names an id that no element has.";
    W503 => Error, "Prop cannot be set by a patch.";
    W504 => Error, "Patch names a slot the parent does not declare.";
    W505 => Error, "Patch index is outside the target list.";
    W506 => Error, "Element moved into its own subtree.";
    W507 => Error, "The root element cannot be removed or moved.";
    W508 => Error, "Inserted markup is not a list of elements.";
    W509 => Error, "Inserted markup reuses an id of the document.";
    W510 => Error, "An added context entry reuses an id of the document.";
    W511 => Error, "A context patch names no context entry.";
    W512 => Error, "The host does not allow this context patch.";
    W601 => Error, "Imported input cannot be read.";
    W602 => Warning, "Imported input exceeds an import limit.";
    W701 => Error, "Project file or one of its members has the wrong shape.";
    W702 => Warning, "Unknown member in the project file.";
    W703 => Error, "File name in the project file is not allowed.";
    W704 => Error, "File named by the project cannot be read.";
    W705 => Error, "Problem in the project's token files.";
    W706 => Error, "Catalog extension is not a valid catalog.";
    W707 => Error, "Catalog extension narrows or changes the core catalog.";
    W708 => Error, "Project action name breaks the action grammar.";
    W709 => Error, "Data schema is malformed.";
    W710 => Warning, "Data schema keyword is not supported.";
    W711 => Error, "Two catalogs claim the same name, prefix or kind.";
    W712 => Error, "Catalog prefix is malformed or reserved, or a second catalog lacks one.";
    W713 => Error, "Catalog defines or extends a kind it does not own.";
    W714 => Warning, "Catalog requirement is not loaded at a compatible version.";
    W801 => Mode, "`<use>` names no fragment of the project.";
    W802 => Error, "Attribute, action or slot the fragment does not declare.";
    W803 => Error, "Malformed `<param>` or `<fragment>`.";
    W804 => Error, "`<outlet>` names no slot parameter, or one twice.";
    W805 => Error, "Fragments use each other in a cycle.";
    W806 => Error, "Expanding the fragments exceeds the element or depth limit.";
    W807 => Error, "Parameter read where its type cannot go.";
    W809 => Error, "Variant parameter or `<variant>` is misdeclared.";
    W810 => Error, "Declared version is lower than the changes require.";
}

impl Code {
    pub fn severity(self, mode: Mode) -> Severity {
        match (self.registered(), mode) {
            (Registered::Error, _) | (Registered::Mode, Mode::Strict) => Severity::Error,
            (Registered::Warning, _) | (Registered::Mode, Mode::Lenient) => Severity::Warning,
        }
    }
}

impl Serialize for Code {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: Code,
    pub severity: Severity,
    pub message: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Always set: a repairing model needs to know what would have been valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Diagnostic {
    /// A diagnostic in lenient mode; `mode` re-resolves the severity of mode-dependent codes.
    pub fn new(
        code: Code,
        path: impl Into<String>,
        message: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code,
            severity: code.severity(Mode::Lenient),
            message: message.into(),
            path: path.into(),
            line: None,
            column: None,
            expected: Some(expected.into()),
            got: None,
            hint: None,
        }
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.severity = self.code.severity(mode);
        self
    }

    pub fn pos(mut self, pos: Option<Position>) -> Self {
        if let Some(p) = pos {
            self.line = Some(p.line);
            self.column = Some(p.column);
        }
        self
    }

    pub fn got(mut self, got: impl Into<String>) -> Self {
        self.got = Some(got.into());
        self
    }

    pub fn got_opt(mut self, got: Option<String>) -> Self {
        self.got = got;
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn hint_opt(mut self, hint: Option<String>) -> Self {
        self.hint = hint;
        self
    }
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Stable sort by position; diagnostics without one sort first, as in the TypeScript core.
pub fn sort_by_position(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_key(|d| (d.line.unwrap_or(0), d.column.unwrap_or(0)));
}

/// `JSON.stringify` of a value, as diagnostics quote what they got.
pub fn quote<T: Serialize + ?Sized>(value: &T) -> String {
    to_compact(value)
}

pub fn one_of<I, S>(values: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let list: Vec<String> = values.into_iter().map(|v| quote_str(v.as_ref())).collect();
    if list.is_empty() {
        "nothing".to_owned()
    } else {
        format!("one of: {}", list.join(", "))
    }
}

/// Nearest candidate by edit distance, close enough to be a likely typo. Lengths and distances
/// count UTF-16 code units, as in the TypeScript core.
pub fn nearest<I, S>(word: &str, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let units: Vec<u16> = word.encode_utf16().collect();
    let lower = word.to_lowercase();
    let len = units.len();
    let limit = if len <= 3 { 1 } else { (len / 3).max(2) };
    let mut best: Option<String> = None;
    let mut best_distance = usize::MAX;
    for candidate in candidates {
        let candidate = candidate.as_ref();
        if candidate == word {
            continue;
        }
        if candidate.to_lowercase() == lower {
            return Some(candidate.to_owned());
        }
        let distance = levenshtein(&units, &candidate.encode_utf16().collect::<Vec<_>>());
        if distance < best_distance {
            best = Some(candidate.to_owned());
            best_distance = distance;
        }
    }
    best.filter(|_| best_distance <= limit)
}

pub fn did_you_mean<I, S>(word: &str, candidates: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    nearest(word, candidates).map(|m| format!("did you mean {}?", quote_str(&m)))
}

fn levenshtein(a: &[u16], b: &[u16]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut current = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + cost);
        }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_codes_warn_when_lenient_and_fail_when_strict() {
        assert_eq!(Code::W401.severity(Mode::Lenient), Severity::Warning);
        assert_eq!(Code::W401.severity(Mode::Strict), Severity::Error);
        assert_eq!(Code::W602.severity(Mode::Strict), Severity::Warning);
        assert_eq!(Code::W101.severity(Mode::Lenient), Severity::Error);
    }

    #[test]
    fn suggestions_allow_small_typos_only() {
        let states = ["idle", "submitting", "invalid"];
        assert_eq!(
            did_you_mean("submiting", states).as_deref(),
            Some("did you mean \"submitting\"?")
        );
        assert_eq!(nearest("IDLE", states).as_deref(), Some("idle"));
        assert_eq!(nearest("zzz", states), None);
        assert_eq!(nearest("idle", states), None);
    }

    #[test]
    fn one_of_quotes_each_value() {
        assert_eq!(one_of(["a", "b"]), r#"one of: "a", "b""#);
        assert_eq!(one_of(Vec::<String>::new()), "nothing");
    }
}
