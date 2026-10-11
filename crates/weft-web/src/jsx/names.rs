//! The names the generated module reserves and the prop tables it maps.

// ---- Names ----

/// A loop variable that would shadow the component's parameters, a keyword or a global the
/// output relies on is renamed; loop variables never contain `_`, so the renamed form is free.
pub(super) const RESERVED: &[&str] = &[
    "arguments",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "data",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "of",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "actions",
    "onChange",
    // What the Lit module imports.
    "html",
    "nothing",
    "styleMap",
    "unsafeStatic",
];

pub(super) const ALIGN: &[(&str, &str)] = &[
    ("start", "flex-start"),
    ("center", "center"),
    ("end", "flex-end"),
    ("stretch", "stretch"),
];

pub(super) const BUSY_STATES: &[&str] = &["loading", "busy", "submitting"];
pub(super) const INPUT_TYPES: &[&str] = &["text", "email", "password", "number", "search"];
pub(super) const SORTS: &[&str] = &["none", "ascending", "descending"];
