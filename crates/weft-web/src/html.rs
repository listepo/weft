//! Weft document → one static HTML page with its CSS and no script, written in the corpus HTML
//! conventions (corpus/README.md): `data-bind="prop:$.path"` for bindings, `data-action="event:name"`
//! for events, `<template data-each data-as>` for repetition and `<template data-empty>` for an
//! empty slot, `data-variant`, `data-state` and `data-tone` for presentation states, and
//! `data-weft-id` on every element that has an id. The page is a template: bound values are left
//! for a host to fill in. Every design token becomes a CSS custom property `--weft-<path>`.
//!
//! Given sample data (`to_html_with_data`), the page shows it instead: the document is filled in
//! first (`fill.rs`), so bindings and repetition are already literal content, and an `empty` slot
//! shows when its list has nothing to show.
//!
//! The document is untrusted. Its strings reach the page only as escaped text or attribute values,
//! URLs only when they are http(s), mailto or relative, and CSS only through the token value shapes
//! below.

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_catalog::{Appearance, Token, composite_part, font_weight, token_types};
use weft_core::{
    Catalog, Child, ComponentDef, Content, Diagnostic, Document, Mode, Node, ValidateOptions,
    Value, format_value, has_errors, js_number, serialize, validate_document,
};

use crate::fill::fill;
use crate::provenance;

pub struct HtmlOptions<'a> {
    pub catalog: &'a Catalog,
    /// Resolved design tokens (`weft_catalog::load_tokens`); each becomes a custom property.
    pub tokens: &'a IndexMap<String, Token>,
    /// Leave the canonical source in a leading comment, so `import_html` gives the document back.
    pub source: bool,
    /// The light and dark tokens of the project's resolver (`weft_catalog::appearance`): the page
    /// follows the system appearance (`tokens_css`).
    pub appearance: Option<Appearance<'a>>,
}

/// The document does not validate in strict mode against the catalog and tokens.
#[derive(Debug)]
pub struct Invalid(pub Vec<Diagnostic>);

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the document has {} diagnostics", self.0.len())
    }
}

impl std::error::Error for Invalid {}

/// The page for `document`.
pub fn to_html(document: &Document, options: &HtmlOptions<'_>) -> Result<String, Invalid> {
    to_html_with_data(document, options, None)
}

/// The page for `document`, showing `data` when given (SPEC §9). The data is untrusted like the
/// document: its strings reach the page through the same escaping and URL rules.
pub fn to_html_with_data(
    document: &Document,
    options: &HtmlOptions<'_>,
    data: Option<&Json>,
) -> Result<String, Invalid> {
    let types = token_types(options.tokens);
    let diagnostics = validate_document(
        document,
        &ValidateOptions {
            catalog: Some(options.catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    if has_errors(&diagnostics) {
        return Err(Invalid(diagnostics));
    }
    let mut out = String::from("<!doctype html>\n");
    if options.source {
        out.push_str(&provenance::comment(
            "<!--",
            "",
            &serialize(document),
            "-->",
        ));
        out.push('\n');
    }
    // The source comment above keeps the screen as written, not as filled in.
    let filled = data.map(|data| fill(document, options.catalog, data));
    let document = filled.as_ref().unwrap_or(document);
    let title = match document.root.props.get("label") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => document.root.id.clone().unwrap_or_else(|| "Weft".into()),
    };
    out.push_str("<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(&format!("<title>{}</title>\n", escape_text(&title)));
    out.push_str("<style>\n");
    out.push_str(&tokens_css(options.tokens, options.appearance));
    out.push_str(LAYOUT_CSS);
    out.push_str("</style>\n</head>\n<body>\n");
    let mut w = Writer {
        catalog: options.catalog,
        out: String::new(),
        sample: data.is_some(),
    };
    w.node(&document.root, &Ctx::default(), 0);
    out.push_str(&w.out);
    out.push_str("</body>\n</html>\n");
    Ok(out)
}

// ---- CSS ----

/// The layout every page shares; the classes and data attributes are the ones `Writer` emits.
const LAYOUT_CSS: &str = "\
.weft-stack { display: flex; flex-direction: column; }
.weft-row { display: flex; flex-direction: row; }
.weft-grid { display: grid; }
[data-align=\"start\"] { align-items: flex-start; }
[data-align=\"center\"] { align-items: center; }
[data-align=\"end\"] { align-items: flex-end; }
[data-align=\"stretch\"] { align-items: stretch; }
[data-wrap] { flex-wrap: wrap; }
.weft-field label, .weft-toggle { display: flex; gap: 0.5em; }
.weft-field label { flex-direction: column; }
[hidden] { display: none !important; }
";

pub(crate) fn token_var(path: &str) -> Option<String> {
    let valid = !path.is_empty()
        && path.split('.').all(|s| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-'))
        });
    valid.then(|| format!("--weft-{}", path.replace('.', "-").replace('$', "_")))
}

/// A token's CSS value, for the shapes that cannot carry anything but a value: a px or rem
/// dimension, a hex colour, a finite number. Anything else is left out.
pub(crate) fn token_css(token: &Token) -> Option<String> {
    let v = &token.value;
    match token.kind.as_str() {
        "dimension" => dimension(v),
        "color" => {
            // The string form of earlier DTCG drafts is still common (the example project's).
            let hex = v.as_str().or_else(|| v.get("hex")?.as_str())?;
            let digits = hex.strip_prefix('#')?;
            ((3..=8).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_hexdigit()))
                .then(|| hex.to_owned())
        }
        "number" => match v {
            Json::Number(n) => n.as_f64().filter(|n| n.is_finite()).map(js_number),
            _ => None,
        },
        _ => None,
    }
}

fn dimension(v: &Json) -> Option<String> {
    let n = v.get("value")?.as_f64().filter(|n| n.is_finite())?;
    let unit = v.get("unit")?.as_str()?;
    matches!(unit, "px" | "rem").then(|| format!("{}{unit}", js_number(n)))
}

/// The custom properties of one token: `--weft-<path>` with its value, and for a typography
/// token the `font` shorthand (weight, size, line height, families) and, when it has letter
/// spacing, `--weft-<path>-letter-spacing`, which the shorthand cannot hold.
fn declarations(
    path: &str,
    token: &Token,
    tokens: &IndexMap<String, Token>,
) -> Vec<(String, String)> {
    let Some(name) = token_var(path) else {
        return vec![];
    };
    if token.kind != "typography" {
        return token_css(token)
            .map(|v| vec![(name, v)])
            .unwrap_or_default();
    }
    let Some((font, spacing)) = typography(&token.value, tokens) else {
        return vec![];
    };
    let mut out = vec![(name.clone(), font)];
    if let Some(spacing) = spacing {
        out.push((format!("{name}-letter-spacing"), spacing));
    }
    out
}

/// The CSS generic families (CSS Fonts 4) and the one vendor keyword browsers still read; any
/// other family is quoted.
const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
    "-apple-system",
];

/// A typography token as the `font` shorthand and its letter spacing, or `None` when a part has
/// no CSS form. Families are CSS strings with every character that could end the string, the
/// declaration or the `<style>` element escaped.
fn typography(v: &Json, tokens: &IndexMap<String, Token>) -> Option<(String, Option<String>)> {
    let part = |key: &str| composite_part(v, key, tokens);
    let size = dimension(&part("fontSize")?)?;
    let families: Vec<String> = match part("fontFamily")? {
        Json::String(s) => vec![s],
        Json::Array(list) => list
            .iter()
            .map(|f| f.as_str().map(str::to_owned))
            .collect::<Option<_>>()?,
        _ => return None,
    };
    if families.is_empty() || families.iter().any(String::is_empty) {
        return None;
    }
    let families: Vec<String> = families
        .iter()
        .map(|f| {
            if GENERIC_FAMILIES.contains(&f.as_str()) {
                f.clone()
            } else {
                css_string(f)
            }
        })
        .collect();
    let mut font = vec![];
    if v.get("fontWeight").is_some() {
        font.push(js_number(font_weight(&part("fontWeight")?)?));
    }
    match v.get("lineHeight") {
        Some(_) => {
            let line = part("lineHeight")?
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0)?;
            font.push(format!("{size}/{}", js_number(line)));
        }
        None => font.push(size),
    }
    font.push(families.join(", "));
    let spacing = match v.get("letterSpacing") {
        Some(_) => Some(dimension(&part("letterSpacing")?)?),
        None => None,
    };
    Some((font.join(" "), spacing))
}

fn css_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // A hex escape ends at the space; `<` would let `</style>` close the element.
            c if c.is_control() || c == '<' => out.push_str(&format!("\\{:x} ", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The tokens as CSS custom properties on `:root` (SPEC §9), the stylesheet of the static page
/// and of `weft css-tokens`. With an appearance (SPEC §10.3) the page follows the system:
/// `color-scheme: light dark`, the light values on `:root`, and the values that differ in the
/// dark context in an `@media (prefers-color-scheme: dark)` block. Without one the output is as
/// it always was.
pub fn tokens_css(tokens: &IndexMap<String, Token>, appearance: Option<Appearance<'_>>) -> String {
    let mut css = String::from(":root {\n");
    if appearance.is_some() {
        css.push_str("  color-scheme: light dark;\n");
    }
    let mut dark = vec![];
    for (path, token) in tokens {
        // A context's token stands in only with the same type: the loader reports the others.
        let in_context = |set: &'_ IndexMap<String, Token>| {
            set.get(path)
                .filter(|t| t.kind == token.kind)
                .map(|t| declarations(path, t, set))
        };
        let light = appearance
            .and_then(|a| in_context(a.light))
            .unwrap_or_else(|| declarations(path, token, tokens));
        if let Some(night) = appearance.and_then(|a| in_context(a.dark)) {
            dark.extend(night.into_iter().filter(|d| !light.contains(d)));
        }
        for (name, value) in light {
            css.push_str(&format!("  {name}: {value};\n"));
        }
    }
    css.push_str("}\n");
    if !dark.is_empty() {
        css.push_str("@media (prefers-color-scheme: dark) {\n  :root {\n");
        for (name, value) in dark {
            css.push_str(&format!("    {name}: {value};\n"));
        }
        css.push_str("  }\n}\n");
    }
    css
}

// ---- Escaping and values ----

pub(crate) fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            _ => out.push(c),
        }
    }
    out
}

/// SPEC §9 trust rule: http, https, mailto or relative; anything else is dropped. Tabs, newlines
/// and surrounding control characters are removed first, as a URL parser would.
pub(crate) fn safe_url(value: &str) -> Option<String> {
    let url: String = value
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let url = url.trim_matches(|c: char| c <= ' ');
    if url.is_empty() {
        return None;
    }
    let colon = url.find(':');
    let delimiter = url.find(['/', '?', '#']);
    let has_scheme = matches!((colon, delimiter), (Some(c), Some(d)) if c < d)
        || matches!((colon, delimiter), (Some(_), None));
    if !has_scheme {
        return Some(url.to_owned());
    }
    let scheme = &url[..colon.unwrap_or_default()];
    let valid = scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let scheme = scheme.to_ascii_lowercase();
    (valid && matches!(scheme.as_str(), "http" | "https" | "mailto")).then(|| url.to_owned())
}

fn literal_text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(js_number(*n)),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn is_true(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Bool(true)))
}

/// `$.path` or `!$.path`, as `data-bind` writes a binding.
fn binding(v: &Value) -> Option<String> {
    match v {
        Value::Bind { bind, not } => Some(format!("{}{bind}", if *not { "!" } else { "" })),
        _ => None,
    }
}

// ---- Elements ----

/// An attribute list; `None` is a boolean attribute.
#[derive(Default)]
struct Attrs(Vec<(String, Option<String>)>);

impl Attrs {
    fn set(&mut self, name: &str, value: impl Into<String>) {
        self.0.push((name.to_owned(), Some(value.into())));
    }

    fn flag(&mut self, name: &str) {
        self.0.push((name.to_owned(), None));
    }

    fn print(&self) -> String {
        let mut out = String::new();
        for (name, value) in &self.0 {
            match value {
                None => out.push_str(&format!(" {name}")),
                Some(v) => out.push_str(&format!(" {name}=\"{}\"", escape_attr(v))),
            }
        }
        out
    }
}

#[derive(Clone, Default)]
struct Ctx {
    /// The id of the enclosing radio group (radio `name`) and its literal value.
    group: Option<(String, Option<String>)>,
    /// The literal value of the enclosing select.
    select: Option<String>,
}

/// What `base` already wrote, so the generic pass leaves it alone.
struct Base {
    attrs: Attrs,
    binds: Vec<String>,
}

const VOID: &[&str] = &["input", "img"];

struct Writer<'a> {
    catalog: &'a Catalog,
    out: String,
    /// The document was filled in with sample data: show the `empty` slot instead of templating it.
    sample: bool,
}

impl Writer<'_> {
    fn line(&mut self, depth: usize, text: &str) {
        for _ in 0..depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn open(&mut self, depth: usize, tag: &str, attrs: &Attrs) {
        self.line(depth, &format!("<{tag}{}>", attrs.print()));
    }

    fn close(&mut self, depth: usize, tag: &str) {
        self.line(depth, &format!("</{tag}>"));
    }

    /// An element whose only content is `text` (already escaped), on one line.
    fn inline(&mut self, depth: usize, tag: &str, attrs: &Attrs, text: &str) {
        if VOID.contains(&tag) {
            self.line(depth, &format!("<{tag}{}>", attrs.print()));
        } else {
            self.line(depth, &format!("<{tag}{}>{text}</{tag}>", attrs.print()));
        }
    }

    /// The attributes every element carries: id, state, hidden, label (unless `caption` shows
    /// it), the events, and every prop not in `handled` (bindings in `data-bind`, other values as
    /// `data-prop-<name>`).
    fn base(&self, n: &Node, handled: &[&str], aria_label: bool) -> Base {
        let mut attrs = Attrs::default();
        let mut binds = Vec::new();
        if let Some(id) = &n.id {
            attrs.set("data-weft-id", id.clone());
        }
        for (name, value) in &n.props {
            if let Some(b) = binding(value) {
                binds.push(format!("{name}:{b}"));
                continue;
            }
            if handled.contains(&name.as_str()) {
                continue;
            }
            match name.as_str() {
                "state" => attrs.set("data-state", literal_text(Some(value)).unwrap_or_default()),
                "hidden" => {
                    if is_true(Some(value)) {
                        attrs.flag("hidden");
                    }
                }
                "label" if aria_label => {
                    attrs.set("aria-label", literal_text(Some(value)).unwrap_or_default());
                }
                "label" => {}
                "role" => attrs.set("role", literal_text(Some(value)).unwrap_or_default()),
                _ => attrs.set(&format!("data-prop-{name}"), format_value(value)),
            }
        }
        Base { attrs, binds }
    }

    fn finish(n: &Node, base: Base) -> Attrs {
        let Base { mut attrs, binds } = base;
        if !binds.is_empty() {
            attrs.set("data-bind", binds.join("; "));
        }
        if !n.on.is_empty() {
            let actions: Vec<String> = n.on.iter().map(|(e, a)| format!("{e}:{a}")).collect();
            attrs.set("data-action", actions.join("; "));
        }
        attrs
    }

    fn def(&self, n: &Node) -> Option<&ComponentDef> {
        self.catalog.components.get(&n.kind)
    }

    /// The text a text-content kind shows: its content, else its literal `text` prop.
    fn shown(n: &Node) -> String {
        let mut s = String::new();
        for c in &n.children {
            if let Child::Text(t) = c {
                s.push_str(t);
            }
        }
        if s.is_empty() {
            s = literal_text(n.props.get("text")).unwrap_or_default();
        }
        escape_text(&s)
    }

    fn label(n: &Node) -> String {
        literal_text(n.props.get("label")).unwrap_or_default()
    }

    fn node(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        if n.kind == "each" {
            return self.each(n, ctx, depth);
        }
        let Some(def) = self.def(n).cloned() else {
            return self.extension(n, ctx, depth);
        };
        match n.kind.as_str() {
            "screen" => self.container(n, ctx, depth, "main", &[]),
            "stack" | "grid" => self.layout(n, ctx, depth),
            "section" => self.container(n, ctx, depth, "section", &[]),
            "form" => self.container(n, ctx, depth, "form", &[]),
            "heading" => {
                let level = match n.props.get("level") {
                    Some(Value::Number(l)) if l.fract() == 0.0 && (1.0..=6.0).contains(l) => {
                        *l as u8
                    }
                    _ => 2,
                };
                let a = Self::finish(n, self.base(n, &["level", "text"], true));
                self.inline(depth, &format!("h{level}"), &a, &Self::shown(n));
            }
            "text" => {
                let mut b = self.base(n, &["text", "tone"], false);
                if let Some(tone) = literal_text(n.props.get("tone")) {
                    b.attrs.set("data-tone", tone);
                }
                let a = Self::finish(n, b);
                self.inline(depth, "p", &a, &Self::shown(n));
            }
            "image" => {
                let mut b = self.base(n, &["src"], false);
                b.attrs.set("alt", Self::label(n));
                if let Some(src) = literal_text(n.props.get("src")).and_then(|s| safe_url(&s)) {
                    b.attrs.set("src", src);
                }
                let a = Self::finish(n, b);
                self.inline(depth, "img", &a, "");
            }
            "link" => {
                let mut b = self.base(n, &["href", "text"], true);
                // A link needs an href to be a link; a bound or unsafe one stays in the page as
                // `#` (the binding is in data-bind).
                let href = literal_text(n.props.get("href")).and_then(|s| safe_url(&s));
                b.attrs.set("href", href.unwrap_or_else(|| "#".into()));
                let a = Self::finish(n, b);
                self.inline(depth, "a", &a, &Self::shown(n));
            }
            "button" | "menu-item" => {
                let mut b = self.base(n, &["variant", "disabled", "submit", "text"], true);
                let submit = is_true(n.props.get("submit"));
                b.attrs
                    .set("type", if submit { "submit" } else { "button" });
                if n.kind == "menu-item" {
                    b.attrs.set("role", "menuitem");
                }
                if let Some(v) = literal_text(n.props.get("variant")) {
                    b.attrs.set("data-variant", v);
                }
                if is_true(n.props.get("disabled")) {
                    b.attrs.flag("disabled");
                }
                let a = Self::finish(n, b);
                self.inline(depth, "button", &a, &Self::shown(n));
            }
            "field" => self.field(n, depth),
            "checkbox" | "switch" => self.toggle(n, depth),
            "radio-group" => {
                let mut b = self.base(n, &["value"], false);
                b.attrs.set("role", "radiogroup");
                let a = Self::finish(n, b);
                self.open(depth, "fieldset", &a);
                let label = Self::label(n);
                if !label.is_empty() {
                    self.inline(depth + 1, "legend", &Attrs::default(), &escape_text(&label));
                }
                let inner = Ctx {
                    group: Some((
                        n.id.clone().unwrap_or_default(),
                        literal_text(n.props.get("value")),
                    )),
                    ..ctx.clone()
                };
                self.children(n, &inner, depth + 1);
                self.close(depth, "fieldset");
            }
            "radio" => self.radio(n, ctx, depth),
            "select" => {
                let mut b = self.base(n, &["value", "disabled"], false);
                if is_true(n.props.get("disabled")) {
                    b.attrs.flag("disabled");
                }
                let a = Self::finish(n, b);
                self.open(depth, "label", &Attrs::default());
                self.inline(
                    depth + 1,
                    "span",
                    &Attrs::default(),
                    &escape_text(&Self::label(n)),
                );
                self.open(depth + 1, "select", &a);
                let inner = Ctx {
                    select: literal_text(n.props.get("value")),
                    ..ctx.clone()
                };
                self.children(n, &inner, depth + 2);
                self.close(depth + 1, "select");
                self.close(depth, "label");
            }
            "option" => {
                let mut b = self.base(n, &["value", "text"], true);
                let value = literal_text(n.props.get("value")).unwrap_or_default();
                if ctx.select.as_deref() == Some(value.as_str()) {
                    b.attrs.flag("selected");
                }
                b.attrs.set("value", value);
                let a = Self::finish(n, b);
                self.inline(depth, "option", &a, &Self::shown(n));
            }
            "list" => {
                let tag = if is_true(n.props.get("ordered")) {
                    "ol"
                } else {
                    "ul"
                };
                let a = Self::finish(n, self.base(n, &["ordered"], true));
                self.open(depth, tag, &a);
                if self.shows_empty(n) {
                    let mut li = Attrs::default();
                    li.set("role", "none");
                    li.set("data-weft-slot", "empty");
                    self.open(depth + 1, "li", &li);
                    self.list(
                        n.slots.get("empty").map_or(&[], Vec::as_slice),
                        ctx,
                        depth + 2,
                    );
                    self.close(depth + 1, "li");
                } else {
                    self.children(n, ctx, depth + 1);
                    self.empty_slot(n, ctx, depth + 1);
                }
                self.close(depth, tag);
            }
            "item" | "cell" | "alert" => {
                let (tag, mut b) = match n.kind.as_str() {
                    "item" => ("li", self.base(n, &["text"], true)),
                    "cell" => ("td", self.base(n, &["text"], true)),
                    _ => ("div", self.base(n, &["text", "tone"], true)),
                };
                if n.kind == "alert" {
                    b.attrs.set("role", "alert");
                    if let Some(tone) = literal_text(n.props.get("tone")) {
                        b.attrs.set("data-tone", tone);
                    }
                }
                let a = Self::finish(n, b);
                self.mixed(n, ctx, depth, tag, &a);
            }
            "table" => self.table(n, ctx, depth),
            "column" => {
                let mut b = self.base(n, &["sort", "text"], true);
                b.attrs.set("scope", "col");
                if let Some(sort) = literal_text(n.props.get("sort")) {
                    b.attrs.set("aria-sort", sort);
                }
                let a = Self::finish(n, b);
                self.inline(depth, "th", &a, &Self::shown(n));
            }
            "row" => {
                let mut b = self.base(n, &["selected"], true);
                if let Some(Value::Bool(s)) = n.props.get("selected") {
                    b.attrs.set("aria-selected", s.to_string());
                }
                let a = Self::finish(n, b);
                self.open(depth, "tr", &a);
                self.children(n, ctx, depth + 1);
                self.close(depth, "tr");
            }
            "tabs" => self.tabs(n, ctx, depth),
            "dialog" => {
                let mut b = self.base(n, &["open", "modal"], true);
                if is_true(n.props.get("open")) {
                    b.attrs.flag("open");
                }
                if !matches!(n.props.get("modal"), Some(Value::Bool(false))) {
                    b.attrs.set("aria-modal", "true");
                }
                let a = Self::finish(n, b);
                self.container_with(n, ctx, depth, "dialog", a);
            }
            "menu" => {
                let mut b = self.base(n, &[], true);
                b.attrs.set("role", "menu");
                let a = Self::finish(n, b);
                self.container_with(n, ctx, depth, "div", a);
            }
            // A kind a custom catalog adds: its role on a container, its content as it comes.
            _ => {
                let mut b = self.base(n, &[], true);
                b.attrs.set("data-weft-kind", n.kind.clone());
                if def.role != "none" && def.role != "generic" {
                    b.attrs.set("role", def.role.clone());
                }
                let a = Self::finish(n, b);
                if def.content == Content::Text {
                    self.inline(depth, "div", &a, &Self::shown(n));
                } else {
                    self.container_with(n, ctx, depth, "div", a);
                }
            }
        }
    }

    fn container(&mut self, n: &Node, ctx: &Ctx, depth: usize, tag: &str, handled: &[&str]) {
        let a = Self::finish(n, self.base(n, handled, true));
        self.container_with(n, ctx, depth, tag, a);
    }

    /// Children, with each named slot in its own box: `header` first, the rest after.
    fn container_with(&mut self, n: &Node, ctx: &Ctx, depth: usize, tag: &str, a: Attrs) {
        self.open(depth, tag, &a);
        if let Some(list) = n.slots.get("header") {
            self.slot(depth + 1, "header", "header", list, ctx);
        }
        self.children(n, ctx, depth + 1);
        for (name, list) in &n.slots {
            if name == "header" {
                continue;
            }
            let tag = if matches!(name.as_str(), "footer" | "actions") {
                "footer"
            } else {
                "div"
            };
            self.slot(depth + 1, tag, name, list, ctx);
        }
        self.close(depth, tag);
    }

    fn slot(&mut self, depth: usize, tag: &str, name: &str, list: &[Child], ctx: &Ctx) {
        let mut a = Attrs::default();
        a.set("data-weft-slot", name);
        self.open(depth, tag, &a);
        self.list(list, ctx, depth + 1);
        self.close(depth, tag);
    }

    /// SPEC §5.1: on a filled-in page, a declared `empty` slot replaces the items when there are
    /// none or the state says so. Loops are already expanded, so any element but a column counts.
    fn shows_empty(&self, n: &Node) -> bool {
        let declared = self.def(n).is_some_and(|d| d.slot("empty").is_some());
        self.sample
            && declared
            && n.slots.contains_key("empty")
            && (literal_text(n.props.get("state")).as_deref() == Some("empty")
                || !n
                    .children
                    .iter()
                    .any(|c| matches!(c, Child::Node(x) if x.kind != "column")))
    }

    fn empty_slot(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        if self.sample {
            return;
        }
        if let Some(list) = n.slots.get("empty") {
            let mut a = Attrs::default();
            a.flag("data-empty");
            self.open(depth, "template", &a);
            self.list(list, ctx, depth + 1);
            self.close(depth, "template");
        }
    }

    fn children(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        self.list(&n.children, ctx, depth);
    }

    fn list(&mut self, list: &[Child], ctx: &Ctx, depth: usize) {
        for c in list {
            match c {
                Child::Node(child) => self.node(child, ctx, depth),
                Child::Text(t) => self.line(depth, &escape_text(t)),
            }
        }
    }

    fn mixed(&mut self, n: &Node, ctx: &Ctx, depth: usize, tag: &str, a: &Attrs) {
        if n.children.iter().all(|c| matches!(c, Child::Text(_))) && n.slots.is_empty() {
            self.inline(depth, tag, a, &Self::shown(n));
        } else {
            self.open(depth, tag, a);
            self.children(n, ctx, depth + 1);
            self.close(depth, tag);
        }
    }

    fn each(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let mut a = Attrs::default();
        if let Some(id) = &n.id {
            a.set("data-weft-id", id.clone());
        }
        if let Some(Value::Bind { bind, .. }) = n.props.get("in") {
            a.set("data-each", bind.clone());
        }
        if let Some(name) = literal_text(n.props.get("as")) {
            a.set("data-as", name);
        }
        self.open(depth, "template", &a);
        self.children(n, ctx, depth + 1);
        self.close(depth, "template");
    }

    fn extension(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let mut b = self.base(n, &[], true);
        b.attrs.set("data-weft-kind", n.kind.clone());
        let a = Self::finish(n, b);
        self.container_with(n, ctx, depth, "div", a);
    }

    fn layout(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let mut b = self.base(n, &["direction", "gap", "align", "wrap", "columns"], false);
        let mut style = Vec::new();
        let class = if n.kind == "grid" {
            if let Some(Value::Number(c)) = n.props.get("columns") {
                style.push(format!(
                    "grid-template-columns: repeat({}, minmax(0, 1fr))",
                    js_number(*c)
                ));
            }
            "weft-grid"
        } else if literal_text(n.props.get("direction")).as_deref() == Some("row") {
            "weft-row"
        } else {
            "weft-stack"
        };
        b.attrs.set("class", class);
        if let Some(Value::Token(t)) = n.props.get("gap")
            && let Some(var) = token_var(t)
        {
            style.push(format!("gap: var({var})"));
        }
        if let Some(align) = literal_text(n.props.get("align")) {
            b.attrs.set("data-align", align);
        }
        if is_true(n.props.get("wrap")) {
            b.attrs.flag("data-wrap");
        }
        if !style.is_empty() {
            b.attrs.set("style", style.join("; "));
        }
        let a = Self::finish(n, b);
        self.container_with(n, ctx, depth, "div", a);
    }

    fn field(&mut self, n: &Node, depth: usize) {
        let ty = literal_text(n.props.get("type")).unwrap_or_else(|| "text".into());
        let multiline = ty == "multiline";
        let mut b = self.base(
            n,
            &[
                "type",
                "value",
                "placeholder",
                "required",
                "disabled",
                "error",
            ],
            false,
        );
        if !multiline {
            b.attrs.set("type", ty);
        }
        if let Some(p) = literal_text(n.props.get("placeholder")) {
            b.attrs.set("placeholder", p);
        }
        if is_true(n.props.get("required")) {
            b.attrs.flag("required");
        }
        if is_true(n.props.get("disabled")) {
            b.attrs.flag("disabled");
        }
        let error = literal_text(n.props.get("error")).filter(|e| !e.is_empty());
        let invalid =
            error.is_some() || literal_text(n.props.get("state")).as_deref() == Some("invalid");
        if invalid {
            b.attrs.set("aria-invalid", "true");
        }
        let error_id = n.id.as_ref().map(|id| format!("weft-{id}-error"));
        if let (Some(_), Some(eid)) = (&error, &error_id) {
            b.attrs.set("aria-describedby", eid.clone());
        }
        let value = literal_text(n.props.get("value"));
        if !multiline && let Some(v) = &value {
            b.attrs.set("value", v.clone());
        }
        let a = Self::finish(n, b);
        let mut wrapper = Attrs::default();
        wrapper.set("class", "weft-field");
        self.open(depth, "div", &wrapper);
        self.open(depth + 1, "label", &Attrs::default());
        self.inline(
            depth + 2,
            "span",
            &Attrs::default(),
            &escape_text(&Self::label(n)),
        );
        if multiline {
            self.inline(
                depth + 2,
                "textarea",
                &a,
                &escape_text(&value.unwrap_or_default()),
            );
        } else {
            self.inline(depth + 2, "input", &a, "");
        }
        self.close(depth + 1, "label");
        if let Some(e) = error {
            let mut ea = Attrs::default();
            if let Some(eid) = error_id {
                ea.set("id", eid);
            }
            ea.set("class", "weft-error");
            self.inline(depth + 1, "p", &ea, &escape_text(&e));
        }
        self.close(depth, "div");
    }

    fn toggle(&mut self, n: &Node, depth: usize) {
        let mut b = self.base(n, &["checked", "disabled"], false);
        b.attrs.set("type", "checkbox");
        if n.kind == "switch" {
            b.attrs.set("role", "switch");
        }
        if is_true(n.props.get("checked")) {
            b.attrs.flag("checked");
        }
        if is_true(n.props.get("disabled")) {
            b.attrs.flag("disabled");
        }
        let a = Self::finish(n, b);
        let mut wrapper = Attrs::default();
        wrapper.set("class", "weft-toggle");
        self.open(depth, "label", &wrapper);
        self.inline(depth + 1, "input", &a, "");
        self.inline(
            depth + 1,
            "span",
            &Attrs::default(),
            &escape_text(&Self::label(n)),
        );
        self.close(depth, "label");
    }

    fn radio(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let mut b = self.base(n, &["value", "disabled", "text"], true);
        b.attrs.set("type", "radio");
        if let Some((group, _)) = &ctx.group
            && !group.is_empty()
        {
            b.attrs.set("name", format!("weft-{group}"));
        }
        let value = literal_text(n.props.get("value")).unwrap_or_default();
        if ctx
            .group
            .as_ref()
            .and_then(|(_, v)| v.as_deref())
            .is_some_and(|v| v == value && !v.is_empty())
        {
            b.attrs.flag("checked");
        }
        b.attrs.set("value", value);
        if is_true(n.props.get("disabled")) {
            b.attrs.flag("disabled");
        }
        let a = Self::finish(n, b);
        let mut wrapper = Attrs::default();
        wrapper.set("class", "weft-toggle");
        self.open(depth, "label", &wrapper);
        self.inline(depth + 1, "input", &a, "");
        self.inline(depth + 1, "span", &Attrs::default(), &Self::shown(n));
        self.close(depth, "label");
    }

    /// SPEC §5.1: `column` children form the header row; other content goes into the body, a
    /// child that is not a row wrapped in a row and cell so the HTML parser keeps it in place.
    fn table(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let a = Self::finish(n, self.base(n, &[], true));
        self.open(depth, "table", &a);
        let is_column = |c: &Child| matches!(c, Child::Node(x) if x.kind == "column");
        let columns: Vec<&Child> = n.children.iter().filter(|c| is_column(c)).collect();
        if !columns.is_empty() {
            self.open(depth + 1, "thead", &Attrs::default());
            self.open(depth + 2, "tr", &Attrs::default());
            for &c in &columns {
                self.list(std::slice::from_ref(c), ctx, depth + 3);
            }
            self.close(depth + 2, "tr");
            self.close(depth + 1, "thead");
        }
        self.open(depth + 1, "tbody", &Attrs::default());
        if self.shows_empty(n) {
            let mut tr = Attrs::default();
            tr.set("data-weft-slot", "empty");
            let mut td = Attrs::default();
            td.set("colspan", columns.len().max(1).to_string());
            self.open(depth + 2, "tr", &tr);
            self.open(depth + 3, "td", &td);
            self.list(
                n.slots.get("empty").map_or(&[], Vec::as_slice),
                ctx,
                depth + 4,
            );
            self.close(depth + 3, "td");
            self.close(depth + 2, "tr");
            self.close(depth + 1, "tbody");
            self.close(depth, "table");
            return;
        }
        for c in n.children.iter().filter(|c| !is_column(c)) {
            match c {
                Child::Node(x) if x.kind == "row" || x.kind == "each" => {
                    self.node(x, ctx, depth + 2);
                }
                other => {
                    self.open(depth + 2, "tr", &Attrs::default());
                    self.open(depth + 3, "td", &Attrs::default());
                    self.list(std::slice::from_ref(other), ctx, depth + 4);
                    self.close(depth + 3, "td");
                    self.close(depth + 2, "tr");
                }
            }
        }
        self.empty_slot(n, ctx, depth + 2);
        self.close(depth + 1, "tbody");
        self.close(depth, "table");
    }

    /// SPEC §5.1: a tablist of tab buttons, then one tabpanel per tab; only the selected panel
    /// is shown. Tabs inside `<each>` repeat in both places.
    fn tabs(&mut self, n: &Node, ctx: &Ctx, depth: usize) {
        let selected = literal_text(n.props.get("selected"));
        let tabs = tab_nodes(&n.children);
        let chosen = tabs
            .iter()
            .position(|t| t.id.is_some() && t.id == selected)
            .unwrap_or(0);
        let mut b = self.base(n, &["selected"], true);
        b.attrs.set("role", "tablist");
        let a = Self::finish(n, b);
        self.open(depth, "div", &Attrs::default());
        self.open(depth + 1, "div", &a);
        let mut index = 0;
        self.tab_part(&n.children, ctx, depth + 2, chosen, &mut index, false);
        self.close(depth + 1, "div");
        let mut index = 0;
        self.tab_part(&n.children, ctx, depth + 1, chosen, &mut index, true);
        self.close(depth, "div");
    }

    fn tab_part(
        &mut self,
        list: &[Child],
        ctx: &Ctx,
        depth: usize,
        chosen: usize,
        index: &mut usize,
        panels: bool,
    ) {
        for c in list {
            let Child::Node(t) = c else { continue };
            if t.kind == "each" {
                let mut a = Attrs::default();
                if let (Some(id), false) = (&t.id, panels) {
                    a.set("data-weft-id", id.clone());
                }
                if let Some(Value::Bind { bind, .. }) = t.props.get("in") {
                    a.set("data-each", bind.clone());
                }
                if let Some(name) = literal_text(t.props.get("as")) {
                    a.set("data-as", name);
                }
                self.open(depth, "template", &a);
                self.tab_part(&t.children, ctx, depth + 1, chosen, index, panels);
                self.close(depth, "template");
                continue;
            }
            if t.kind != "tab" {
                if !panels {
                    self.node(t, ctx, depth);
                }
                continue;
            }
            let id = t.id.clone().unwrap_or_default();
            let on = *index == chosen;
            *index += 1;
            if panels {
                let mut a = Attrs::default();
                a.set("role", "tabpanel");
                a.set("id", format!("weft-{id}-panel"));
                a.set("aria-labelledby", format!("weft-{id}-tab"));
                if !on {
                    a.flag("hidden");
                }
                self.open(depth, "div", &a);
                self.children(t, ctx, depth + 1);
                for (name, slot) in &t.slots {
                    self.slot(depth + 1, "div", name, slot, ctx);
                }
                self.close(depth, "div");
            } else {
                let mut b = self.base(t, &[], false);
                b.attrs.set("type", "button");
                b.attrs.set("role", "tab");
                b.attrs.set("id", format!("weft-{id}-tab"));
                b.attrs.set("aria-selected", on.to_string());
                b.attrs.set("aria-controls", format!("weft-{id}-panel"));
                let a = Self::finish(t, b);
                self.inline(depth, "button", &a, &escape_text(&Self::label(t)));
            }
        }
    }
}

fn tab_nodes(list: &[Child]) -> Vec<&Node> {
    let mut out = Vec::new();
    for c in list {
        if let Child::Node(n) = c {
            if n.kind == "tab" {
                out.push(n.as_ref());
            } else if n.kind == "each" {
                out.extend(tab_nodes(&n.children));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_urls_survive() {
        assert_eq!(safe_url("https://a.b/c").as_deref(), Some("https://a.b/c"));
        assert_eq!(safe_url("/x?y:z").as_deref(), Some("/x?y:z"));
        assert_eq!(safe_url("MAILTO:a@b").as_deref(), Some("MAILTO:a@b"));
        assert_eq!(safe_url("java\nscript:alert(1)"), None);
        assert_eq!(safe_url(" data:text/html,x"), None);
        assert_eq!(safe_url(""), None);
    }

    #[test]
    fn tokens_become_custom_properties_of_safe_shapes_only() {
        let token = |kind: &str, value: Json| Token {
            kind: kind.into(),
            value,
        };
        assert_eq!(
            token_css(&token(
                "dimension",
                serde_json::json!({"value": 0.5, "unit": "rem"})
            )),
            Some("0.5rem".into())
        );
        assert_eq!(
            token_css(&token(
                "dimension",
                serde_json::json!({"value": 1, "unit": "vw"})
            )),
            None
        );
        assert_eq!(
            token_css(&token("color", serde_json::json!({"hex": "#fff;}"}))),
            None
        );
        assert_eq!(
            token_css(&token("fontFamily", serde_json::json!("x"))),
            None
        );
        assert_eq!(
            token_var("color.blue.$root").as_deref(),
            Some("--weft-color-blue-_root")
        );
        assert_eq!(token_var("a;b"), None);
        assert_eq!(
            token_css(&token("color", serde_json::json!("#d97706"))),
            Some("#d97706".into())
        );
        assert_eq!(token_css(&token("color", serde_json::json!("red;x"))), None);
    }

    fn set(pairs: &[(&str, &str, Json)]) -> IndexMap<String, Token> {
        pairs
            .iter()
            .map(|(path, kind, value)| {
                let token = Token {
                    kind: (*kind).into(),
                    value: value.clone(),
                };
                ((*path).to_owned(), token)
            })
            .collect()
    }

    #[test]
    fn typography_is_a_font_shorthand_and_its_letter_spacing() {
        let tokens = set(&[
            (
                "size",
                "dimension",
                serde_json::json!({"value": 1, "unit": "rem"}),
            ),
            (
                "body",
                "typography",
                serde_json::json!({
                    "fontFamily": ["Inter\"</style>", "system-ui"],
                    "fontSize": "{size}",
                    "fontWeight": "semi-bold",
                    "letterSpacing": {"value": 0.2, "unit": "px"},
                    "lineHeight": 1.5
                }),
            ),
            (
                "bad",
                "typography",
                serde_json::json!({"fontFamily": "x", "fontSize": {"value": 1, "unit": "vw"}}),
            ),
        ]);
        assert_eq!(
            tokens_css(&tokens, None),
            ":root {\n  --weft-size: 1rem;\n  --weft-body: 600 1rem/1.5 \"Inter\\\"\\3c /style>\", system-ui;\n  --weft-body-letter-spacing: 0.2px;\n}\n"
        );
    }

    #[test]
    fn an_appearance_overrides_what_differs_in_the_dark() {
        let light = set(&[
            ("ink", "color", serde_json::json!("#000000")),
            (
                "gap",
                "dimension",
                serde_json::json!({"value": 8, "unit": "px"}),
            ),
        ]);
        let dark = set(&[
            ("ink", "color", serde_json::json!("#ffffff")),
            (
                "gap",
                "dimension",
                serde_json::json!({"value": 8, "unit": "px"}),
            ),
        ]);
        let appearance = Appearance {
            modifier: "theme",
            light_context: "light",
            dark_context: "dark",
            light: &light,
            dark: &dark,
        };
        // The default context is dark here; `:root` still holds the light values.
        assert_eq!(
            tokens_css(&dark, Some(appearance)),
            ":root {\n  color-scheme: light dark;\n  --weft-ink: #000000;\n  --weft-gap: 8px;\n}\n@media (prefers-color-scheme: dark) {\n  :root {\n    --weft-ink: #ffffff;\n  }\n}\n"
        );
    }
}
