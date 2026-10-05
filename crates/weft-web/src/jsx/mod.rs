//! Weft document → a self-contained React or SolidJS component (SPEC §9, "To JSX"). The mapping
//! restates `@weft/render-react` kind by kind rather than sharing it: the renderer builds elements
//! with closures over resolved values, while this module prints source text, so a shared table
//! would only cover tag names. packages/to-jsx/test/equivalence.test.ts renders both on every
//! corpus screen and catalog example and compares the results, which is what keeps the two from
//! drifting. React and SolidJS share one render plan; they differ only in how a loop, a condition,
//! a dynamic tag and a few attributes are spelled.
//!
//! The document is untrusted. No document string ever reaches the output except through
//! `JSON.stringify` (a JavaScript string literal) or a JSX text/attribute run whose characters are
//! all inert; identifiers come only from this module or are checked against the loop-variable
//! grammar.

mod print;
mod runtime;
mod tree;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use serde_json::{Map, Value as Json};
use weft_core::{
    ARIA_ROLES, Catalog, ComponentDef, Content, PropType, is_binding, is_loop_variable, is_token,
    js_number,
};
use weft_import::{js_number_from, js_trim, squash};

use crate::js::{INHERITED, V, compare_utf16, is_integer, is_plain_segment, js_round, quote};
pub use runtime::{Helper, RUNTIME, react_runtime};
use tree::{
    Attr, AttrV, C, Code, J, Seg, Str, attr, attr_js, attr_s, el, expr, js, lit, s, str_js,
    str_literal,
};

/// Same bound as the renderer's expansion: deeper content is not rendered.
pub const MAX_DEPTH: usize = 200;

/// The framework a component is generated for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Framework {
    #[default]
    React,
    Solid,
}

#[derive(Clone, Copy, Debug)]
pub struct JsxOptions<'a> {
    pub catalog: &'a Catalog,
    /// `WeftScreen` when absent; must match `[A-Z][A-Za-z0-9]*`.
    pub component_name: Option<&'a str>,
    pub framework: Framework,
    /// TSX: typed props and helpers.
    pub typescript: bool,
    /// Leave the canonical source in a leading comment, so `import_jsx` gives the document back.
    pub source: bool,
}

/// The component name is not an identifier the output could carry safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadComponentName;

impl fmt::Display for BadComponentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("componentName must match /^[A-Z][A-Za-z0-9]*$/")
    }
}

impl std::error::Error for BadComponentName {}

fn is_component_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The component for `document`, which may be any JSON: what is not a document renders nothing.
pub fn to_jsx(document: &Json, options: &JsxOptions<'_>) -> Result<String, BadComponentName> {
    let name = options.component_name.unwrap_or("WeftScreen");
    if !is_component_name(name) {
        return Err(BadComponentName);
    }
    let mut g = Gen::new(options.catalog, options.framework, options.typescript);
    let scope = Rc::new(Scope::default());
    let root = document.as_object().and_then(|o| o.get("root"));
    let pieces = match root {
        Some(root) => g.pieces_of(std::slice::from_ref(root), &scope, 0),
        None => Vec::new(),
    };
    let first = pieces.into_iter().find_map(|p| match p {
        Piece::Node(n, hidden) => Some((n, hidden)),
        _ => None,
    });
    let mut body = "null".to_owned();
    if let Some((n, hidden)) = first
        && let Some(c) = g.render(&n, &Ctx::default())
    {
        body = g.root_body(c, hidden);
    }
    let module = g.module(name, &body);
    match options
        .source
        .then(|| source_comment(document, options, name))
        .flatten()
    {
        Some(comment) => Ok(format!("{comment}\n{module}")),
        None => Ok(module),
    }
}

/// The `weft:source` comment: the framework, the component name and the flavour on its first
/// line, so the importer can regenerate the same module. Only a document of the right shape has
/// a source to leave.
fn source_comment(document: &Json, options: &JsxOptions<'_>, name: &str) -> Option<String> {
    let diagnostics = weft_core::validate(
        document,
        &weft_core::ValidateOptions {
            catalog: Some(options.catalog),
            mode: weft_core::Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    if weft_core::has_errors(&diagnostics) {
        return None;
    }
    let markup = weft_core::serialize(&weft_core::canonicalize(&weft_core::to_document(document)));
    let framework = match options.framework {
        Framework::React => "react",
        Framework::Solid => "solid",
    };
    let flavour = if options.typescript {
        " typescript"
    } else {
        ""
    };
    let head = format!("{framework} name={name}{flavour}");
    Some(crate::provenance::comment("/*", &head, &markup, "*/"))
}

// ---- Compile-time values ----

/// A value known at compile time (`lit`) or computed by `js` at run time; `bool` marks an
/// expression that already yields a boolean.
#[derive(Clone, Debug)]
struct E {
    js: String,
    lit: Option<V>,
    bool: bool,
}

fn lit_e(v: V) -> E {
    E {
        js: v.literal(),
        lit: Some(v),
        bool: false,
    }
}

fn dyn_e(js: impl Into<String>, bool: bool) -> E {
    E {
        js: js.into(),
        lit: None,
        bool,
    }
}

impl E {
    fn is(&self, v: &V) -> bool {
        self.lit.as_ref() == Some(v)
    }
}

const TRUE: V = V::Bool(true);
const FALSE: V = V::Bool(false);

fn str_v(s: impl Into<String>) -> V {
    V::Str(s.into())
}

/// A number prop brought into its declared range (SPEC §5.1), as the renderer's `prop` does.
pub(crate) fn clamp_number(v: &V, integer: bool, min: Option<f64>, max: Option<f64>) -> V {
    let V::Num(x) = v else {
        return v.clone();
    };
    if !x.is_finite() {
        return V::Undef;
    }
    let mut out = if integer { js_round(*x) } else { *x };
    if let Some(min) = min {
        out = out.max(min);
    }
    if let Some(max) = max {
        out = out.min(max);
    }
    V::Num(out)
}

fn opt_literal(n: Option<f64>) -> String {
    n.map_or_else(|| "undefined".to_owned(), |n| V::Num(n).literal())
}

// ---- Names ----

/// A loop variable that would shadow the component's parameters, a keyword or a global the
/// output relies on is renamed; loop variables never contain `_`, so the renamed form is free.
const RESERVED: &[&str] = &[
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
];

const ALIGN: &[(&str, &str)] = &[
    ("start", "flex-start"),
    ("center", "center"),
    ("end", "flex-end"),
    ("stretch", "stretch"),
];

const BUSY_STATES: &[&str] = &["loading", "busy", "submitting"];
const INPUT_TYPES: &[&str] = &["text", "email", "password", "number", "search"];
const SORTS: &[&str] = &["none", "ascending", "descending"];

// ---- Scope and pieces ----

#[derive(Clone)]
struct Var {
    ident: String,
    path: Str,
}

#[derive(Clone, Default)]
struct Scope {
    vars: HashMap<String, Var>,
    suffix: Str,
    item: Option<Str>,
    loops: usize,
}

/// A node of the document as the renderer's expansion reads it.
pub(crate) struct N<'a> {
    kind: &'a str,
    def: Option<&'a ComponentDef>,
    doc_id: &'a str,
    id: Str,
    props: Option<&'a Map<String, Json>>,
    on: Option<&'a Map<String, Json>>,
    children: Option<&'a Json>,
    slots: Option<&'a Map<String, Json>>,
    scope: Rc<Scope>,
    depth: usize,
}

impl<'a> N<'a> {
    fn raw(&self, name: &str) -> Option<&'a Json> {
        self.props?.get(name)
    }

    fn event(&self, name: &str) -> Option<&'a str> {
        self.on?.get(name)?.as_str()
    }

    fn slot(&self, name: &str) -> Option<&'a Json> {
        self.slots?.get(name)
    }
}

enum Piece<'a> {
    Text(String),
    /// A node and, when it depends on data, the expression that hides it.
    Node(Rc<N<'a>>, Option<String>),
    Each {
        src: String,
        ident: String,
        index: String,
        inner: Vec<Piece<'a>>,
    },
}

/// A piece that is not a loop, as the per-piece callbacks see it.
#[derive(Clone, Copy)]
enum Leaf<'p, 'a> {
    Text(&'p str),
    Node(&'p Rc<N<'a>>),
}

/// `group` is the enclosing radio-group, `form` the nearest enclosing form.
#[derive(Clone, Default)]
struct Ctx<'a> {
    group: Option<Rc<N<'a>>>,
    form: Option<Rc<N<'a>>>,
}

fn is_binding_raw(raw: Option<&Json>) -> bool {
    raw.and_then(Json::as_object)
        .is_some_and(|o| o.get("bind").is_some_and(Json::is_string))
}

/// Strings of a `text`-content kind, joined as the renderer's `contentText` joins them.
fn content_text(list: Option<&Json>) -> String {
    let Some(Json::Array(items)) = list else {
        return String::new();
    };
    let parts: Vec<&str> = items
        .iter()
        .filter_map(Json::as_str)
        .map(js_trim)
        .filter(|s| !s.is_empty())
        .collect();
    squash(&parts.join(" "))
}

fn filter_pieces<'a>(list: &[Piece<'a>], keep: &dyn Fn(Leaf<'_, 'a>) -> bool) -> Vec<Piece<'a>> {
    let mut out = Vec::new();
    for p in list {
        match p {
            Piece::Text(text) => {
                if keep(Leaf::Text(text)) {
                    out.push(Piece::Text(text.clone()));
                }
            }
            Piece::Node(n, hidden) => {
                if keep(Leaf::Node(n)) {
                    out.push(Piece::Node(n.clone(), hidden.clone()));
                }
            }
            Piece::Each {
                src,
                ident,
                index,
                inner,
            } => {
                let inner = filter_pieces(inner, keep);
                if !inner.is_empty() {
                    out.push(Piece::Each {
                        src: src.clone(),
                        ident: ident.clone(),
                        index: index.clone(),
                        inner,
                    });
                }
            }
        }
    }
    out
}

type MapFn<'f, 'a> = dyn FnMut(&mut Gen<'a>, Leaf<'_, 'a>) -> Option<C<'a>> + 'f;
type MakeFn<'f, 'a> = dyn FnMut(&mut Gen<'a>, Leaf<'_, 'a>) -> Option<Code<'a>> + 'f;

pub(crate) struct Gen<'a> {
    catalog: &'a Catalog,
    fw: Framework,
    ts: bool,
    used: HashSet<&'static str>,
    fragment: bool,
    /// SolidJS imports, by module.
    solid: BTreeSet<&'static str>,
    solid_web: BTreeSet<&'static str>,
}

impl<'a> Gen<'a> {
    fn new(catalog: &'a Catalog, fw: Framework, ts: bool) -> Self {
        Gen {
            catalog,
            fw,
            ts,
            used: HashSet::new(),
            fragment: false,
            solid: BTreeSet::new(),
            solid_web: BTreeSet::new(),
        }
    }

    fn solid(&self) -> bool {
        self.fw == Framework::Solid
    }

    fn use_(&mut self, helper: &'static str) -> &'static str {
        self.used.insert(helper);
        if helper == "_keyed" {
            self.fragment = true;
        }
        helper
    }

    fn data_ident(&self) -> &'static str {
        if self.solid() { "props.data" } else { "data" }
    }

    fn actions_ident(&self) -> &'static str {
        if self.solid() {
            "props.actions"
        } else {
            "actions"
        }
    }

    fn on_change_ident(&self) -> &'static str {
        if self.solid() {
            "props.onChange"
        } else {
            "onChange"
        }
    }

    fn ident(&self, name: &str) -> String {
        if RESERVED.contains(&name) || (self.solid() && name == "props") {
            format!("{name}_")
        } else {
            name.to_owned()
        }
    }

    /// An attribute name as the framework spells it: SolidJS sets HTML attributes by their HTML
    /// names and has no uncontrolled `default*` props.
    fn an(&self, react: &'static str) -> &'static str {
        if !self.solid() {
            return react;
        }
        match react {
            "tabIndex" => "tabindex",
            "colSpan" => "colspan",
            "defaultValue" => "value",
            "defaultChecked" => "checked",
            other => other,
        }
    }

    /// A style key: React's camelCase property names, SolidJS's CSS property names.
    fn css(&self, react: &str, css: &str) -> String {
        if self.solid() && react != css {
            quote(css)
        } else {
            react.to_owned()
        }
    }

    // ---- Values (SPEC §2.1, as resolved by the renderer) ----

    fn resolve(&mut self, raw: Option<&Json>, scope: &Scope) -> (E, Option<Str>) {
        let Some(Json::Object(record)) = raw else {
            return (lit_e(V::of(raw)), None);
        };
        let Some(Json::String(bind)) = record.get("bind") else {
            return (lit_e(V::Undef), None);
        };
        if !is_binding(bind) {
            return (lit_e(V::Undef), None);
        }
        let (name, rest) = match bind.find('.') {
            Some(dot) => (&bind[1..dot], &bind[dot..]),
            None => (&bind[1..], ""),
        };
        let segments: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest[1..].split('.').collect()
        };
        let (base, path) = if name.is_empty() {
            (self.data_ident().to_owned(), vec![s("$")])
        } else {
            let Some(v) = scope.vars.get(name) else {
                return (lit_e(V::Undef), None);
            };
            (v.ident.clone(), v.path.clone())
        };
        let access = if segments
            .iter()
            .all(|s| is_plain_segment(s) && INHERITED.binary_search(s).is_err())
        {
            let mut a = base;
            for s in &segments {
                a.push_str("?.");
                a.push_str(s);
            }
            a
        } else {
            let get = self.use_("_get");
            format!("{get}({base}, {})", weft_core::to_compact(&segments))
        };
        if record.get("not") == Some(&Json::Bool(true)) {
            let on = self.use_("_on");
            return (dyn_e(format!("!{on}({access})"), true), None);
        }
        let mut full = path;
        full.push(s(rest));
        (dyn_e(access, false), Some(full))
    }

    fn text(&mut self, e: E) -> E {
        match &e.lit {
            Some(v) => lit_e(str_v(v.text())),
            None => {
                let f = self.use_("_text");
                dyn_e(format!("{f}({})", e.js), false)
            }
        }
    }

    fn flag(&mut self, e: E) -> E {
        if let Some(v) = &e.lit {
            return lit_e(V::Bool(v.truthy()));
        }
        if e.bool {
            return e;
        }
        let f = self.use_("_on");
        dyn_e(format!("{f}({})", e.js), true)
    }

    /// A number prop is brought into its declared range as the renderer's `prop` does (SPEC §5.1).
    fn prop(&mut self, n: &N<'a>, name: &str) -> (E, Option<Str>) {
        let (e, path) = self.resolve(n.raw(name), &n.scope);
        let Some(def) = n
            .def
            .and_then(|d| d.prop(name))
            .filter(|d| d.kind == PropType::Number)
        else {
            return (e, path);
        };
        let integer = def.integer == Some(true);
        if let Some(v) = &e.lit {
            return (lit_e(clamp_number(v, integer, def.min, def.max)), path);
        }
        let bounds = [
            V::Bool(integer).literal(),
            opt_literal(def.min),
            opt_literal(def.max),
        ]
        .join(", ");
        let f = self.use_("_num");
        (dyn_e(format!("{f}({}, {bounds})", e.js), false), path)
    }

    fn text_of(&mut self, n: &N<'a>, name: &str) -> E {
        let e = self.prop(n, name).0;
        self.text(e)
    }

    fn flag_of(&mut self, n: &N<'a>, name: &str) -> E {
        let e = self.prop(n, name).0;
        self.flag(e)
    }

    fn label(&mut self, n: &N<'a>) -> E {
        let t = self.text_of(n, "label");
        match &t.lit {
            Some(v) => lit_e(str_v(js_trim(&v.string()))),
            None => dyn_e(format!("{}.trim()", t.js), false),
        }
    }

    /// SPEC §5.1: a text-bearing kind takes its text from content or from the `text` prop. As in
    /// render-react's expansion, the prop counts only when the content renders nothing (content
    /// wins when a lenient reader is given both) and its text is not blank.
    #[inline(never)]
    fn text_prop_shown(&mut self, n: &N<'a>) -> (E, E) {
        let text = self.text_of(n, "text");
        if n.raw("text").is_none() {
            return (text, lit_e(FALSE));
        }
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let content = self.any_shown(&pieces);
        let blank = match &text.lit {
            Some(v) => lit_e(V::Bool(js_trim(&v.text()).is_empty())),
            None => dyn_e(format!("{}.trim() === \"\"", text.js), false),
        };
        if content.is(&TRUE) || blank.is(&TRUE) {
            return (text, lit_e(FALSE));
        }
        let parts: Vec<&str> = [&content, &blank]
            .into_iter()
            .filter(|e| e.lit.is_none())
            .map(|e| e.js.as_str())
            .collect();
        let when = if parts.is_empty() {
            lit_e(TRUE)
        } else {
            dyn_e(format!("!({})", parts.join(" || ")), true)
        };
        (text, when)
    }

    /// The visible text of a `text`-content kind, joined and squashed as `contentText` does.
    fn own_text(&mut self, n: &N<'a>) -> E {
        let content = lit_e(str_v(content_text(n.children)));
        let (text, when) = self.text_prop_shown(n);
        if let Some(w) = &when.lit {
            return if *w == TRUE {
                self.squash(text)
            } else {
                content
            };
        }
        let squashed = self.squash(text);
        dyn_e(
            format!("{} ? {} : {}", when.js, squashed.js, content.js),
            false,
        )
    }

    fn squash(&mut self, e: E) -> E {
        match &e.lit {
            Some(v) => lit_e(str_v(squash(&v.text()))),
            None => {
                let f = self.use_("_squash");
                dyn_e(format!("{f}({})", e.js), false)
            }
        }
    }

    /// A loop over `src` outside JSX, with the loop variable and index in scope of `body`.
    fn loop_call(
        &mut self,
        method: &str,
        src: &str,
        ident: &str,
        index: &str,
        body: &str,
    ) -> String {
        if self.solid() {
            let ix = self.use_("_ix");
            format!("{ix}({src}).{method}(([{ident}, {index}]) => {body}")
        } else {
            format!("{src}.{method}(({ident}, {index}) => {body}")
        }
    }

    /// Whether any of these pieces renders, i.e. whether the renderer's expanded list is
    /// non-empty.
    #[inline(never)]
    fn any_shown(&mut self, list: &[Piece<'a>]) -> E {
        let mut parts = Vec::new();
        for p in list {
            match p {
                Piece::Text(_) | Piece::Node(_, None) => return lit_e(TRUE),
                Piece::Node(_, Some(hidden)) => parts.push(format!("!{hidden}")),
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.any_shown(inner);
                    if inner.is(&FALSE) {
                        continue;
                    }
                    let call = self.loop_call("some", src, ident, index, &inner.js);
                    parts.push(format!("{call})"));
                }
            }
        }
        if parts.is_empty() {
            lit_e(FALSE)
        } else {
            dyn_e(parts.join(" || "), true)
        }
    }

    // ---- Pieces: `each` and `hidden` as the renderer's expansion reads them ----

    fn pieces(
        &mut self,
        list: Option<&'a Json>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> Vec<Piece<'a>> {
        match list {
            Some(Json::Array(items)) => self.pieces_of(items, scope, depth),
            _ => Vec::new(),
        }
    }

    // A stack frame per nesting level stays small only when the compiler does not merge the
    // per-kind code into this function; see `render`.
    #[inline(never)]
    fn pieces_of(&mut self, items: &'a [Json], scope: &Rc<Scope>, depth: usize) -> Vec<Piece<'a>> {
        let mut out: Vec<Piece<'a>> = Vec::new();
        if depth > MAX_DEPTH {
            return out;
        }
        for child in items {
            if let Json::String(text) = child {
                if js_trim(text).is_empty() {
                    continue;
                }
                // Adjacent text is one DOM text node; the renderer joins it with a space.
                if let Some(Piece::Text(last)) = out.last_mut() {
                    last.push(' ');
                    last.push_str(text);
                } else {
                    out.push(Piece::Text(text.clone()));
                }
                continue;
            }
            let Json::Object(record) = child else {
                continue;
            };
            let Some(Json::String(kind)) = record.get("kind") else {
                continue;
            };
            let props = record.get("props").and_then(Json::as_object);
            if kind == "each" {
                if let Some(each) = self.each(record, props, scope, depth) {
                    out.push(each);
                }
                continue;
            }
            let resolved = self.resolve(props.and_then(|p| p.get("hidden")), scope).0;
            let hidden = self.flag(resolved);
            if hidden.is(&TRUE) {
                continue;
            }
            let n = self.node(record, kind, props, scope, depth);
            out.push(Piece::Node(
                Rc::new(n),
                hidden.lit.is_none().then_some(hidden.js),
            ));
        }
        out
    }

    fn each(
        &mut self,
        node: &'a Map<String, Json>,
        props: Option<&'a Map<String, Json>>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> Option<Piece<'a>> {
        let (source, source_path) = self.resolve(props.and_then(|p| p.get("in")), scope);
        let as_ = props.and_then(|p| p.get("as")).and_then(Json::as_str);
        let (Some(mut path), Some(as_)) = (source_path, as_) else {
            return None;
        };
        if !is_loop_variable(as_) {
            return None;
        }
        let index = format!("_i{}", scope.loops);
        // SolidJS's `<For>` passes the index as an accessor.
        let index_ref = if self.solid() {
            format!("{index}()")
        } else {
            index.clone()
        };
        path.push(s("."));
        path.push(js(index_ref.clone()));
        let v = Var {
            ident: self.ident(as_),
            path: path.clone(),
        };
        let ident = v.ident.clone();
        let mut vars = scope.vars.clone();
        vars.insert(as_.to_owned(), v);
        let mut suffix = scope.suffix.clone();
        suffix.extend([s("["), js(index_ref), s("]")]);
        let inner = Rc::new(Scope {
            vars,
            suffix,
            item: Some(path),
            loops: scope.loops + 1,
        });
        let list = self.use_("_list");
        let src = format!("{list}({})", source.js);
        Some(Piece::Each {
            src,
            ident,
            index,
            inner: self.pieces(node.get("children"), &inner, depth + 1),
        })
    }

    fn node(
        &self,
        raw: &'a Map<String, Json>,
        kind: &'a str,
        props: Option<&'a Map<String, Json>>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> N<'a> {
        let doc_id = raw.get("id").and_then(Json::as_str).unwrap_or("");
        let id = if doc_id.is_empty() {
            Vec::new()
        } else {
            let mut id = vec![s(doc_id)];
            id.extend(scope.suffix.iter().cloned());
            id
        };
        N {
            kind,
            def: self.catalog.components.get(kind),
            doc_id,
            id,
            props,
            on: raw.get("on").and_then(Json::as_object),
            children: raw.get("children"),
            slots: raw.get("slots").and_then(Json::as_object),
            scope: scope.clone(),
            depth,
        }
    }

    fn declares_slot(&self, n: &N<'a>, name: &str) -> bool {
        n.def.and_then(|d| d.slot(name)).is_some()
    }

    /// A component's content as render.ts `content` places it (SPEC §4.2): the header slot first,
    /// then the default content, then every other slot by name, each named slot in its own box. A
    /// declared `empty` slot is left to the kind that shows it.
    #[inline(never)]
    fn content(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> Vec<C<'a>> {
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut names: Vec<&'a String> = n.slots.map(|s| s.keys().collect()).unwrap_or_default();
        names.sort_by(|a, b| compare_utf16(a, b));
        for name in names {
            if name == "empty" && self.declares_slot(n, name) {
                continue;
            }
            let pieces = self.pieces(n.slot(name), &n.scope, n.depth + 1);
            let kids = self.kids(&pieces, ctx, None);
            let slot = C::J(el(
                "div",
                vec![attr_s("data-weft-slot", name.as_str())],
                kids,
            ));
            if name == "header" {
                before.push(slot);
            } else {
                after.push(slot);
            }
        }
        let own = match n.def.map(|d| d.content) {
            Some(Content::Text | Content::Mixed) => self.text_prop(n),
            _ => Vec::new(),
        };
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let body = self.kids(&pieces, ctx, None);
        before.extend(own);
        before.extend(body);
        before.extend(after);
        before
    }

    /// Whether the default slot has something to show besides empty loops, as render-react's
    /// `shows` decides it (SPEC §5.1 `empty`): a static element counts even when hidden, a loop
    /// only when its array has items, and table columns never count.
    fn filled(&mut self, n: &N<'a>) -> E {
        let mut loops = Vec::new();
        if let Some(Json::Array(items)) = n.children {
            for c in items {
                let Some(record) = c.as_object() else {
                    continue;
                };
                let Some(kind) = record.get("kind").and_then(Json::as_str) else {
                    continue;
                };
                if kind == "column" {
                    continue;
                }
                if kind != "each" {
                    return lit_e(TRUE);
                }
                let source_raw = record
                    .get("props")
                    .and_then(Json::as_object)
                    .and_then(|p| p.get("in"));
                let source = self.resolve(source_raw, &n.scope).0;
                match &source.lit {
                    None => {
                        let list = self.use_("_list");
                        loops.push(format!("{list}({}).length > 0", source.js));
                    }
                    Some(V::Arr(len)) if *len > 0 => return lit_e(TRUE),
                    Some(_) => {}
                }
            }
        }
        if loops.is_empty() {
            lit_e(FALSE)
        } else {
            dyn_e(loops.join(" || "), true)
        }
    }

    /// Whether a declared `empty` slot replaces the default content (render-react
    /// `showsEmpty`).
    fn shows_empty(&mut self, n: &N<'a>) -> E {
        if !self.declares_slot(n, "empty") || n.slot("empty").is_none() {
            return lit_e(FALSE);
        }
        let state = self.text_of(n, "state");
        let filled = self.filled(n);
        if state.is(&str_v("empty")) || filled.is(&FALSE) {
            return lit_e(TRUE);
        }
        if state.lit.is_some() && filled.lit.is_some() {
            return lit_e(FALSE);
        }
        let mut parts = Vec::new();
        if state.lit.is_none() {
            parts.push(format!("{} === \"empty\"", state.js));
        }
        if filled.lit.is_none() {
            parts.push(format!("!({})", filled.js));
        }
        dyn_e(parts.join(" || "), true)
    }

    /// `then` when `when` holds, else `otherwise` (nothing when absent).
    fn cond(&mut self, when: &str, then: C<'a>, otherwise: Option<C<'a>>) -> C<'a> {
        if self.solid() {
            self.solid.insert("Show");
            let mut attrs = vec![attr_js("when", when)];
            if let Some(o) = otherwise {
                attrs.push(attr("fallback", AttrV::Expr(Box::new(o))));
            }
            return C::J(el("Show", attrs, vec![then]));
        }
        let mut segs = vec![lit(format!("{when} ? ")), expr(then), lit(" : ")];
        match otherwise {
            Some(o) => segs.push(expr(o)),
            None => segs.push(lit("null")),
        }
        C::Code(Code::of(segs))
    }

    /// Pieces as JSX children.
    #[inline(never)]
    fn kids(
        &mut self,
        list: &[Piece<'a>],
        ctx: &Ctx<'a>,
        mut map: Option<&mut MapFn<'_, 'a>>,
    ) -> Vec<C<'a>> {
        let mut out = Vec::new();
        for p in list {
            match p {
                Piece::Text(text) => {
                    let c = match map.as_deref_mut() {
                        Some(f) => f(self, Leaf::Text(text)),
                        None => Some(C::Text(text.clone())),
                    };
                    out.extend(c);
                }
                Piece::Node(n, hidden) => {
                    let c = match map.as_deref_mut() {
                        Some(f) => f(self, Leaf::Node(n)),
                        None => self.render(n, ctx),
                    };
                    let Some(c) = c else {
                        continue;
                    };
                    match hidden {
                        None => out.push(c),
                        Some(h) if self.solid() => {
                            out.push(self.cond(&format!("!{h}"), c, None));
                        }
                        Some(h) => {
                            out.push(C::Code(Code::of(vec![lit(format!("!{h} && ")), expr(c)])))
                        }
                    }
                }
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.kids(inner, ctx, map.as_deref_mut());
                    if self.solid() {
                        self.solid.insert("For");
                        let frag = el("", vec![], inner);
                        let body = Code::of(vec![
                            lit(format!("({ident}, {index}) => ")),
                            expr(C::J(frag)),
                        ]);
                        out.push(C::J(el(
                            "For",
                            vec![attr_js("each", src.as_str())],
                            vec![C::Code(body)],
                        )));
                    } else {
                        self.fragment = true;
                        let frag = el("Fragment", vec![attr_js("key", index.as_str())], inner);
                        out.push(C::Code(Code::of(vec![
                            lit(format!("{src}.map(({ident}, {index}) => ")),
                            expr(C::J(frag)),
                            lit(")"),
                        ])));
                    }
                }
            }
        }
        out
    }

    /// Pieces as an array expression, keeping the entries `make` returns.
    #[inline(never)]
    fn collect(&mut self, list: &[Piece<'a>], make: &mut MakeFn<'_, 'a>) -> Code<'a> {
        let mut items = Vec::new();
        for p in list {
            let (leaf, hidden) = match p {
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.collect(inner, make);
                    let head = self.loop_call("flatMap", src, ident, index, "");
                    items.push(Code::of(vec![
                        lit(format!("...{head}")),
                        Seg::Code(inner),
                        lit(")"),
                    ]));
                    continue;
                }
                Piece::Text(text) => (Leaf::Text(text), None),
                Piece::Node(n, hidden) => (Leaf::Node(n), hidden.as_ref()),
            };
            let Some(item) = make(self, leaf) else {
                continue;
            };
            items.push(match hidden {
                None => item,
                Some(h) => Code::of(vec![
                    lit(format!("...({h} ? [] : [")),
                    Seg::Code(item),
                    lit("])"),
                ]),
            });
        }
        Code::of(vec![Seg::Array(items)])
    }

    // ---- Attributes ----

    fn attr(&self, attrs: &mut Vec<Attr<'a>>, name: &str, e: E) {
        match e.lit {
            Some(V::Undef | V::Null) => {}
            Some(V::Str(text)) => attrs.push(attr(name, AttrV::S(text))),
            Some(v) => attrs.push(attr(name, AttrV::Js(v.literal()))),
            None => attrs.push(attr(name, AttrV::Js(e.js))),
        }
    }

    /// An HTML boolean attribute: present when true, absent otherwise.
    fn bool_attr(&self, attrs: &mut Vec<Attr<'a>>, name: &str, e: E) {
        match e.lit {
            None => attrs.push(attr(name, AttrV::Js(e.js))),
            Some(V::Bool(true)) => attrs.push(attr(name, AttrV::True)),
            Some(_) => {}
        }
    }

    fn or_undefined(&self, e: E) -> E {
        match e.lit {
            Some(V::Str(text)) if text.is_empty() => lit_e(V::Undef),
            Some(v) => lit_e(v),
            None => dyn_e(format!("{} || undefined", e.js), false),
        }
    }

    /// The document id and state on every element that carries a declared role (render.ts
    /// `base`).
    #[inline(never)]
    fn base(&mut self, n: &N<'a>, named: bool) -> Vec<Attr<'a>> {
        let mut a = Vec::new();
        if !n.doc_id.is_empty() {
            a.push(attr(
                "data-weft-id",
                match str_literal(&n.id) {
                    Some(text) => AttrV::S(text),
                    None => AttrV::Js(str_js(&n.id)),
                },
            ));
        }
        let state = self.text_of(n, "state");
        let shown = self.or_undefined(state.clone());
        self.attr(&mut a, "data-state", shown);
        match &state.lit {
            Some(v) => {
                if BUSY_STATES.contains(&v.string().as_str()) {
                    a.push(attr_s("aria-busy", "true"));
                }
            }
            None => {
                let busy = self.use_("_busy");
                a.push(attr_js("aria-busy", format!("{busy}({})", state.js)));
            }
        }
        if named {
            let label = self.label(n);
            let shown = self.or_undefined(label);
            self.attr(&mut a, "aria-label", shown);
        }
        a
    }

    fn fire(&mut self, n: &N<'a>, event: &str) -> Option<String> {
        let action = n.event(event)?;
        let item = n
            .scope
            .item
            .as_ref()
            .map(|i| format!(", item: {}", str_js(i)))
            .unwrap_or_default();
        let act = self.use_("_act");
        Some(format!(
            "{act}({}, {{ id: {}, action: {}{item} }})",
            self.actions_ident(),
            str_js(&n.id),
            quote(action)
        ))
    }

    fn write(&mut self, n: &N<'a>, name: &str, value: &str) -> Option<String> {
        let path = self.prop(n, name).1?;
        Some(format!(
            "{}?.({}, {value})",
            self.on_change_ident(),
            str_js(&path)
        ))
    }

    fn handler(params: &str, statements: Vec<Option<String>>) -> String {
        let body: Vec<String> = statements.into_iter().flatten().collect();
        match body.as_slice() {
            [] => format!("{params} => {{}}"),
            [only] => format!("{params} => {only}"),
            _ => format!("{params} => {{ {}; }}", body.join("; ")),
        }
    }

    fn pressable(&mut self, n: &N<'a>, attrs: &mut Vec<Attr<'a>>) {
        let Some(fire) = self.fire(n, "press") else {
            return;
        };
        let press = self.use_("_press");
        attrs.push(attr_js(self.an("tabIndex"), "0"));
        attrs.push(attr_js("onClick", format!("() => {fire}")));
        attrs.push(attr_js(
            "onKeyDown",
            format!("(_e) => {press}(_e, () => {fire})"),
        ));
    }

    // ---- Kinds (render.ts `renderNode`) ----

    // Every kind gets its own frame rather than one frame as large as all of them together, so
    // that a document nested to the depth limit fits the stack of WebAssembly and of threads.
    #[inline(never)]
    fn render(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        if n.def.is_none() {
            return Some(self.fallback(n, ctx));
        }
        Some(match n.kind {
            "screen" => {
                let a = self.base(n, true);
                C::J(el("main", a, self.content(n, ctx)))
            }
            "stack" | "grid" => {
                let mut a = self.base(n, false);
                let style = self.layout_style(n);
                a.push(attr_js("style", style));
                C::J(el("div", a, self.content(n, ctx)))
            }
            "section" => {
                let a = self.base(n, true);
                C::J(el("section", a, self.content(n, ctx)))
            }
            "heading" => self.heading(n),
            "text" => {
                let mut a = self.base(n, false);
                let tone = self.text_of(n, "tone");
                let tone = self.or_undefined(tone);
                self.attr(&mut a, "data-tone", tone);
                let text = self.own_text(n);
                C::J(el("div", a, self.text_kids(text)))
            }
            "image" => {
                let mut a = self.base(n, false);
                let label = self.label(n);
                self.attr(&mut a, "alt", label);
                let src = self.text_of(n, "src");
                let url = self.use_("_url");
                a.push(attr_js("src", format!("{url}({})", src.js)));
                C::J(el("img", a, vec![]))
            }
            "link" => self.link(n),
            "button" => self.button(n, ctx),
            "form" => {
                let mut a = self.base(n, true);
                let submit = self.fire(n, "submit");
                a.push(attr_js(
                    "onSubmit",
                    Self::handler("(_e)", vec![Some("_e.preventDefault()".to_owned()), submit]),
                ));
                let inner = Ctx {
                    form: Some(n.clone()),
                    ..ctx.clone()
                };
                C::J(el("form", a, self.content(n, &inner)))
            }
            "field" => self.field(n),
            "checkbox" | "switch" => self.toggle(n),
            "radio-group" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "radiogroup"));
                let label = self.label(n);
                let mut kids = self.caption(label);
                let inner = Ctx {
                    group: Some(n.clone()),
                    ..ctx.clone()
                };
                kids.extend(self.content(n, &inner));
                C::J(el("div", a, kids))
            }
            "radio" => self.radio(n, ctx),
            "select" => self.select(n),
            "option" => {
                let a = self.base(n, false);
                let text = self.own_text(n);
                C::J(el("div", a, self.text_kids(text)))
            }
            "list" => self.list(n, ctx),
            "item" => {
                let mut a = self.base(n, true);
                self.pressable(n, &mut a);
                C::J(el("li", a, self.content(n, ctx)))
            }
            "table" => self.table(n, ctx),
            "tabs" => self.tabs(n, ctx),
            "tab" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "group"));
                C::J(el("div", a, self.content(n, ctx)))
            }
            "dialog" => return self.dialog(n, ctx),
            "alert" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "alert"));
                let tone = self.text_of(n, "tone");
                let tone = self.or_undefined(tone);
                self.attr(&mut a, "data-tone", tone);
                C::J(el("div", a, self.content(n, ctx)))
            }
            "menu" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "menu"));
                C::J(el("div", a, self.content(n, ctx)))
            }
            "menu-item" => {
                let mut a = self.base(n, true);
                a.push(attr_s("type", "button"));
                a.push(attr_s("role", "menuitem"));
                let disabled = self.flag_of(n, "disabled");
                self.bool_attr(&mut a, "disabled", disabled);
                if let Some(fire) = self.fire(n, "press") {
                    a.push(attr_js("onClick", format!("() => {fire}")));
                }
                let text = self.own_text(n);
                C::J(el("button", a, self.text_kids(text)))
            }
            _ => self.fallback(n, ctx),
        })
    }

    fn text_kids(&self, e: E) -> Vec<C<'a>> {
        match &e.lit {
            Some(V::Str(text)) if text.is_empty() => vec![],
            Some(v) => vec![C::Text(v.string())],
            None => vec![C::Code(Code::lit(e.js))],
        }
    }

    /// The visible caption of a control (render.ts `caption`): the name itself is `aria-label`,
    /// so the caption is hidden from the tree rather than read twice, and absent when there is
    /// none.
    fn caption(&mut self, name: E) -> Vec<C<'a>> {
        let span = el(
            "span",
            vec![attr_s("aria-hidden", "true")],
            self.text_kids(name.clone()),
        );
        match &name.lit {
            Some(V::Str(text)) if text.is_empty() => vec![],
            Some(_) => vec![C::J(span)],
            None => vec![self.cond(&name.js, C::J(span), None)],
        }
    }

    /// The `text` prop as raw content, where a kind's content is rendered as child nodes
    /// (render.ts `kids`) rather than joined like `contentText`.
    fn text_prop(&mut self, n: &N<'a>) -> Vec<C<'a>> {
        let (text, when) = self.text_prop_shown(n);
        match &when.lit {
            Some(w) if *w == TRUE => self.text_kids(text),
            Some(_) => vec![],
            None if self.solid() => {
                vec![self.cond(&when.js, C::Code(Code::lit(text.js)), None)]
            }
            None => vec![C::Code(Code::lit(format!(
                "{} ? {} : null",
                when.js, text.js
            )))],
        }
    }

    /// Unknown kinds and extensions keep their children under their fallback role (SPEC §8).
    #[inline(never)]
    fn fallback(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> C<'a> {
        let declared = match n.def {
            Some(def) => Some(def.role.as_str()),
            None => n.raw("role").and_then(Json::as_str),
        };
        let role = declared
            .filter(|r| ARIA_ROLES.contains(r))
            .unwrap_or("group");
        let kids = self.content(n, ctx);
        if matches!(role, "none" | "presentation" | "generic") {
            let a = self.base(n, false);
            return C::J(el("div", a, kids));
        }
        let mut a = self.base(n, true);
        if role == "region" || role == "form" {
            // Unnamed, both are generic (HTML-AAM), so the role is kept only with a label.
            let label = self.label(n);
            match &label.lit {
                Some(V::Str(text)) if text.is_empty() => {}
                Some(_) => a.push(attr_s("role", role)),
                None => a.push(attr_js(
                    "role",
                    format!("{} ? {} : undefined", label.js, quote(role)),
                )),
            }
        } else {
            a.push(attr_s("role", role));
        }
        C::J(el("div", a, kids))
    }

    #[inline(never)]
    fn layout_style(&mut self, n: &N<'a>) -> String {
        let mut entries = Vec::new();
        if let Some(token) = n
            .raw("gap")
            .and_then(Json::as_object)
            .and_then(|g| g.get("token"))
            .and_then(Json::as_str)
            .filter(|t| is_token(t))
        {
            let value = format!("var(--weft-{})", token.replace('.', "-"));
            entries.push(format!("gap: {}", quote(&value)));
        }
        if n.kind == "grid" {
            entries.push("display: \"grid\"".to_owned());
            let columns = self.prop(n, "columns").0;
            let key = self.css("gridTemplateColumns", "grid-template-columns");
            match &columns.lit {
                Some(V::Num(c)) if is_integer(*c) && (1.0..=64.0).contains(c) => {
                    let value = format!("repeat({}, minmax(0, 1fr))", js_number(*c));
                    entries.push(format!("{key}: {}", quote(&value)));
                }
                Some(_) => {}
                None => {
                    let cols = self.use_("_cols");
                    entries.push(format!("{key}: {cols}({})", columns.js));
                }
            }
            return format!("{{ {} }}", entries.join(", "));
        }
        entries.push("display: \"flex\"".to_owned());
        let direction = self.text_of(n, "direction");
        let key = self.css("flexDirection", "flex-direction");
        entries.push(match &direction.lit {
            Some(v) => format!(
                "{key}: {}",
                if *v == str_v("row") {
                    "\"row\""
                } else {
                    "\"column\""
                }
            ),
            None => format!("{key}: {} === \"row\" ? \"row\" : \"column\"", direction.js),
        });
        let align = self.text_of(n, "align");
        let key = self.css("alignItems", "align-items");
        match &align.lit {
            Some(v) => {
                let k = v.string();
                if let Some((_, value)) = ALIGN.iter().find(|(name, _)| *name == k) {
                    entries.push(format!("{key}: {}", quote(value)));
                }
            }
            None => {
                let f = self.use_("_align");
                entries.push(format!("{key}: {f}({})", align.js));
            }
        }
        let wrap = self.flag_of(n, "wrap");
        let key = self.css("flexWrap", "flex-wrap");
        match &wrap.lit {
            None => entries.push(format!("{key}: {} ? \"wrap\" : undefined", wrap.js)),
            Some(v) if *v == TRUE => entries.push(format!("{key}: \"wrap\"")),
            Some(_) => {}
        }
        format!("{{ {} }}", entries.join(", "))
    }

    /// An element whose tag is chosen at run time; `cast` is the tag TSX checks its attributes
    /// against.
    fn with_tag(&mut self, tag_js: String, cast: &str, mut j: J<'a>) -> C<'a> {
        if self.solid() {
            self.solid_web.insert("Dynamic");
            j.tag = "Dynamic".to_owned();
            j.attrs.insert(0, attr_js("component", tag_js));
            return C::J(j);
        }
        let line = if self.ts {
            format!("const Tag = ({tag_js}) as \"{cast}\";")
        } else {
            format!("const Tag = {tag_js};")
        };
        j.tag = "Tag".to_owned();
        block(vec![Code::lit(line)], C::J(j))
    }

    #[inline(never)]
    fn heading(&mut self, n: &N<'a>) -> C<'a> {
        let a = self.base(n, true);
        let text = self.own_text(n);
        let j = el("", a, self.text_kids(text));
        let level = self.prop(n, "level").0;
        let Some(v) = &level.lit else {
            let f = self.use_("_level");
            return self.with_tag(format!("\"h\" + {f}({})", level.js), "h2", j);
        };
        let num = match v {
            V::Str(text) => Some(js_number_from(text)),
            V::Num(x) => Some(*x),
            _ => None,
        };
        let h = num
            .filter(|x| is_integer(*x) && (1.0..=6.0).contains(x))
            .unwrap_or(2.0);
        C::J(J {
            tag: format!("h{}", js_number(h)),
            ..j
        })
    }

    #[inline(never)]
    fn link(&mut self, n: &N<'a>) -> C<'a> {
        let fire = self.fire(n, "press");
        let mut a = self.base(n, true);
        a.push(attr_js("href", "_href"));
        // Without an href an <a> is neither a link nor focusable; both are restored, and like a
        // native link it then activates on Enter only.
        a.push(attr_js(
            "role",
            "_href === undefined ? \"link\" : undefined",
        ));
        a.push(attr_js(
            self.an("tabIndex"),
            "_href === undefined ? 0 : undefined",
        ));
        if let Some(fire) = fire {
            a.push(attr_js(
                "onKeyDown",
                format!(
                    "_href === undefined ? (_e) => {{ if (_e.key !== \"Enter\" || _e.target !== _e.currentTarget) return; _e.preventDefault(); {fire}; }} : undefined"
                ),
            ));
            a.push(attr_js("onClick", format!("() => {fire}")));
        }
        a.push(attr_js("style", "{ display: \"inline-block\" }"));
        let href = self.text_of(n, "href");
        let url = self.use_("_url");
        let line = format!("const _href = {url}({});", href.js);
        let text = self.own_text(n);
        let kids = self.text_kids(text);
        block(vec![Code::lit(line)], C::J(el("a", a, kids)))
    }

    #[inline(never)]
    fn button(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> C<'a> {
        // `submit` is literal-only (SPEC §5.1), so a binding never turns a button into a submit
        // button.
        let submit = matches!(n.raw("submit"), Some(Json::Bool(true)))
            || matches!(n.raw("submit"), Some(Json::String(s)) if s == "true");
        let mut a = self.base(n, true);
        a.push(attr_s("type", if submit { "submit" } else { "button" }));
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let variant = self.text_of(n, "variant");
        let variant = self.or_undefined(variant);
        self.attr(&mut a, "data-variant", variant);
        // As in render.ts, the browser's own submission (also from Enter in a field) is cancelled
        // and reported here, so press and the form's submit each fire exactly once, in that order.
        let form = if submit { ctx.form.clone() } else { None };
        let press = self.fire(n, "press");
        let send = match &form {
            Some(f) => self.fire(f, "submit"),
            None => None,
        };
        let click = vec![
            submit.then(|| "_e.preventDefault()".to_owned()),
            press,
            send,
        ];
        if click.iter().any(Option::is_some) {
            a.push(attr_js("onClick", Self::handler("(_e)", click)));
        }
        let text = self.own_text(n);
        C::J(el("button", a, self.text_kids(text)))
    }

    #[inline(never)]
    fn field(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let error = self.text_of(n, "error");
        let ty = self.text_of(n, "type");
        let mut error_parts = vec![s("weft-")];
        error_parts.extend(n.id.iter().cloned());
        error_parts.push(s("-error"));
        let error_id = str_js(&error_parts);
        let value = self.text_of(n, "value");
        let mut a = self.base(n, false);
        self.attr(&mut a, "aria-label", name.clone());
        let mut tag = None;
        match &ty.lit {
            Some(v) => {
                let t = v.string();
                if t == "multiline" {
                    tag = Some("textarea");
                } else {
                    tag = Some("input");
                    let shown = if INPUT_TYPES.contains(&t.as_str()) {
                        t
                    } else {
                        "text".to_owned()
                    };
                    a.push(attr_s("type", shown));
                }
            }
            None => a.push(attr_js(
                "type",
                format!(
                    "{t} === \"multiline\" ? undefined : [\"text\", \"email\", \"password\", \"number\", \"search\"].includes({t}) ? {t} : \"text\"",
                    t = ty.js
                ),
            )),
        }
        let placeholder = self.text_of(n, "placeholder");
        let placeholder = self.or_undefined(placeholder);
        self.attr(&mut a, "placeholder", placeholder);
        let required = self.flag_of(n, "required");
        self.bool_attr(&mut a, "required", required);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let state = self.text_of(n, "state");
        match (&state.lit, &error.lit) {
            (Some(st), Some(err)) => {
                let err = err.string();
                if *st == str_v("invalid") || !err.is_empty() {
                    a.push(attr_s("aria-invalid", "true"));
                }
                if !err.is_empty() {
                    a.push(attr_js("aria-describedby", error_id.clone()));
                }
            }
            _ => {
                a.push(attr_js(
                    "aria-invalid",
                    format!(
                        "{} === \"invalid\" || {} !== \"\" ? \"true\" : undefined",
                        state.js, error.js
                    ),
                ));
                a.push(attr_js(
                    "aria-describedby",
                    format!("{} ? {error_id} : undefined", error.js),
                ));
            }
        }
        let change = self.fire(n, "change");
        // React's `onChange` on a text control fires on every input; SolidJS keeps the DOM's
        // meaning, where that event is `input`.
        let on_input = if self.solid() { "onInput" } else { "onChange" };
        if is_binding_raw(n.raw("value")) {
            // A number input shows "" for anything that is not a valid float, so the value is
            // sanitized the same way before the framework sees it.
            let shown = match &ty.lit {
                Some(t) if *t == str_v("number") => {
                    let f = self.use_("_float");
                    format!("{f}({})", value.js)
                }
                Some(_) => value.js.clone(),
                None => {
                    let f = self.use_("_float");
                    format!(
                        "{} === \"number\" ? {f}({}) : {}",
                        ty.js, value.js, value.js
                    )
                }
            };
            a.push(attr_js("value", shown));
            let write = self.write(n, "value", "_e.currentTarget.value");
            a.push(attr_js(
                on_input,
                Self::handler("(_e)", vec![write, change]),
            ));
        } else {
            self.attr(&mut a, self.an("defaultValue"), value);
            if let Some(change) = change {
                a.push(attr_js(on_input, format!("() => {change}")));
            }
        }
        let control = el(tag.unwrap_or("Tag"), a, vec![]);
        let control = match tag {
            Some(_) => C::J(control),
            None => self.with_tag(
                format!("{} === \"multiline\" ? \"textarea\" : \"input\"", ty.js),
                "input",
                control,
            ),
        };
        let error_el = el(
            "div",
            vec![attr_js("id", error_id)],
            self.text_kids(error.clone()),
        );
        let mut label_kids = self.caption(name);
        label_kids.push(control);
        let mut kids = vec![C::J(el("label", vec![], label_kids))];
        match &error.lit {
            Some(v) => {
                if !v.string().is_empty() {
                    kids.push(C::J(error_el));
                }
            }
            None => kids.push(self.cond(&error.js, C::J(error_el), None)),
        }
        C::J(el("div", vec![attr_s("data-weft-field", "")], kids))
    }

    #[inline(never)]
    fn toggle(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let mut a = self.base(n, false);
        a.push(attr_s("type", "checkbox"));
        if n.kind == "switch" {
            a.push(attr_s("role", "switch"));
        }
        self.attr(&mut a, "aria-label", name.clone());
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let checked = self.flag_of(n, "checked");
        let change = self.fire(n, "change");
        if is_binding_raw(n.raw("checked")) {
            a.push(attr_js("checked", checked.js));
            let write = self.write(n, "checked", "_e.currentTarget.checked");
            a.push(attr_js(
                "onChange",
                Self::handler("(_e)", vec![write, change]),
            ));
        } else {
            self.bool_attr(&mut a, self.an("defaultChecked"), checked);
            if let Some(change) = change {
                a.push(attr_js("onChange", format!("() => {change}")));
            }
        }
        let mut kids = vec![C::J(el("input", a, vec![]))];
        kids.extend(self.caption(name));
        C::J(el("label", vec![], kids))
    }

    #[inline(never)]
    fn radio(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let group = ctx.group.clone();
        let label = self.label(n);
        let own = self.own_text(n);
        let (name, shown) = match (&label.lit, &own.lit) {
            (Some(l), Some(o)) => {
                let (l, o) = (l.string(), o.string());
                let name = if l.is_empty() { o.clone() } else { l.clone() };
                let shown = if o.is_empty() { l } else { o };
                (lit_e(str_v(name)), lit_e(str_v(shown)))
            }
            _ => (
                dyn_e(format!("{} || {}", label.js, own.js), false),
                dyn_e(format!("{} || {}", own.js, label.js), false),
            ),
        };
        let value = self.text_of(n, "value");
        let mut a = self.base(n, false);
        a.push(attr_s("type", "radio"));
        if let Some(g) = group.as_ref().filter(|g| !g.doc_id.is_empty()) {
            let mut parts = vec![s("weft-")];
            parts.extend(g.id.iter().cloned());
            a.push(attr_js("name", str_js(&parts)));
        }
        self.attr(&mut a, "value", value.clone());
        self.attr(&mut a, "aria-label", name);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let mut on = lit_e(FALSE);
        if let Some(g) = &group {
            let gv = self.text_of(g, "value");
            on = match (&gv.lit, &value.lit) {
                (Some(gv), Some(v)) => {
                    let gv = gv.string();
                    lit_e(V::Bool(!gv.is_empty() && gv == v.string()))
                }
                _ => dyn_e(
                    format!("{} !== \"\" && {} === {}", gv.js, value.js, gv.js),
                    true,
                ),
            };
        }
        let controlled = group
            .as_ref()
            .is_some_and(|g| is_binding_raw(g.raw("value")));
        if controlled {
            a.push(attr_js("checked", on.js));
        } else {
            self.bool_attr(&mut a, self.an("defaultChecked"), on);
        }
        if let Some(g) = &group {
            let own_value = self.prop(n, "value").0;
            let statements = vec![
                self.write(g, "value", &own_value.js),
                self.fire(g, "change"),
            ];
            if controlled || statements.iter().any(Option::is_some) {
                a.push(attr_js("onChange", Self::handler("()", statements)));
            }
        }
        // The visible text is the content; a `label` only overrides the accessible name.
        let span = el(
            "span",
            vec![attr_s("aria-hidden", "true")],
            self.text_kids(shown),
        );
        C::J(el(
            "label",
            vec![],
            vec![C::J(el("input", a, vec![])), C::J(span)],
        ))
    }

    /// Only options are rendered: the HTML parser drops anything else inside <select>.
    #[inline(never)]
    fn select(&mut self, n: &N<'a>) -> C<'a> {
        let value = self.text_of(n, "value");
        // SolidJS's server renderer writes a select's value as an attribute HTML ignores, so each
        // option says whether it is the chosen one; the select's own value stays for the client.
        let chosen = self.solid().then(|| value.clone());
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let options = self.collect(&pieces, &mut |g, leaf| {
            let Leaf::Node(o) = leaf else {
                return None;
            };
            if o.kind != "option" {
                return None;
            }
            let mut a = g.base(o, true);
            let value = g.text_of(o, "value");
            if let Some(chosen) = &chosen {
                match (&value.lit, &chosen.lit) {
                    (Some(mine), Some(theirs)) => {
                        if mine.string() == theirs.string() {
                            a.push(attr("selected", AttrV::True));
                        }
                    }
                    _ => a.push(attr_js(
                        "selected",
                        format!("{} === {}", value.js, chosen.js),
                    )),
                }
            }
            g.attr(&mut a, "value", value);
            let text = g.own_text(o);
            let option = el("option", a, g.text_kids(text));
            Some(Code::of(vec![
                lit("{ value: "),
                Seg::Value(o.clone()),
                lit(", el: "),
                expr(C::J(option)),
                lit(" }"),
            ]))
        });
        let mut a = self.base(n, false);
        let label = self.label(n);
        self.attr(&mut a, "aria-label", label);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let controlled = is_binding_raw(n.raw("value"));
        if controlled {
            a.push(attr_js("value", value.js));
        } else {
            self.attr(&mut a, self.an("defaultValue"), value);
        }
        let write = self.write(n, "value", "_c ? _c.value : _e.currentTarget.value");
        let change = self.fire(n, "change");
        if write.is_some() {
            let text = self.use_("_text");
            let find =
                format!("const _c = _o.find((_x) => {text}(_x.value) === _e.currentTarget.value)");
            a.push(attr_js(
                "onChange",
                Self::handler("(_e)", vec![Some(find), write, change]),
            ));
        } else if controlled || change.is_some() {
            a.push(attr_js("onChange", Self::handler("()", vec![change])));
        }
        let shown = if self.solid() {
            "_o.map((_x) => _x.el)".to_owned()
        } else {
            let keyed = self.use_("_keyed");
            format!("{keyed}(_o.map((_x) => _x.el))")
        };
        let control = block(
            vec![Code::of(vec![
                lit("const _o = "),
                Seg::Code(options),
                lit(";"),
            ])],
            C::J(el("select", a, vec![C::Code(Code::lit(shown))])),
        );
        let label = self.label(n);
        let mut kids = self.caption(label);
        kids.push(control);
        C::J(el("label", vec![], kids))
    }

    /// SPEC §5.1: the `empty` slot is shown instead of the items when there are none, or when
    /// the state says `empty`.
    #[inline(never)]
    fn list(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let ordered = self.flag_of(n, "ordered");
        let content = self.content(n, ctx);
        let show = self.shows_empty(n);
        let mut kids = content;
        if !show.is(&FALSE) {
            // A list may only hold list items, so the empty content stands in a presentational
            // one.
            let pieces = self.pieces(n.slot("empty"), &n.scope, n.depth + 1);
            let empty_kids = self.kids(&pieces, ctx, None);
            let empty = el(
                "li",
                vec![attr_s("role", "none"), attr_s("data-weft-slot", "empty")],
                empty_kids,
            );
            kids = if show.lit.is_some() {
                vec![C::J(empty)]
            } else {
                let items = el("", vec![], kids);
                vec![self.cond(&show.js, C::J(empty), Some(C::J(items)))]
            };
        }
        let a = self.base(n, true);
        let j = el("", a, kids);
        match &ordered.lit {
            Some(v) => C::J(J {
                tag: if *v == TRUE { "ol" } else { "ul" }.to_owned(),
                ..j
            }),
            None => self.with_tag(format!("{} ? \"ol\" : \"ul\"", ordered.js), "ul", j),
        }
    }

    /// SPEC §5.1: `column` children form the header row in <thead>; other content goes to
    /// <tbody>, wrapped in a row and cell when it is not a row.
    #[inline(never)]
    fn table(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let children = self.pieces(n.children, &n.scope, n.depth + 1);
        let columns = self.collect(&children, &mut |g, leaf| match leaf {
            Leaf::Node(c) if c.kind == "column" => {
                let column = g.column(c);
                Some(Code::of(vec![expr(C::J(column))]))
            }
            _ => None,
        });
        let rows = self.collect(&children, &mut |g, leaf| {
            let c = match leaf {
                Leaf::Node(p) if p.kind == "column" => return None,
                Leaf::Text(text) => Some(C::J(el(
                    "tr",
                    vec![],
                    vec![C::J(el("td", vec![], vec![C::Text(text.to_owned())]))],
                ))),
                Leaf::Node(p) if p.kind == "row" => Some(C::J(g.row(p, ctx))),
                Leaf::Node(p) => {
                    let rendered = g.render(p, ctx);
                    cell_wrap(rendered)
                }
            };
            c.map(|c| Code::of(vec![expr(c)]))
        });
        let (cols, rows_js) = if self.solid() {
            ("_cols".to_owned(), "_rows".to_owned())
        } else {
            let keyed = self.use_("_keyed");
            (format!("{keyed}(_cols)"), format!("{keyed}(_rows)"))
        };
        let head = el(
            "thead",
            vec![],
            vec![C::J(el("tr", vec![], vec![C::Code(Code::lit(cols))]))],
        );
        let body = el("tbody", vec![], vec![C::Code(Code::lit(rows_js))]);
        let mut kids = vec![self.cond("_cols.length > 0", C::J(head), None)];
        let show = self.shows_empty(n);
        if show.is(&FALSE) {
            kids.push(self.cond("_rows.length > 0", C::J(body), None));
        } else {
            // A table keeps its column headers; the empty content spans them in one body row.
            let pieces = self.pieces(n.slot("empty"), &n.scope, n.depth + 1);
            let empty_kids = self.kids(&pieces, ctx, None);
            let empty = el(
                "tbody",
                vec![],
                vec![C::J(el(
                    "tr",
                    vec![attr_s("data-weft-slot", "empty")],
                    vec![C::J(el(
                        "td",
                        vec![attr_js(self.an("colSpan"), "Math.max(1, _cols.length)")],
                        empty_kids,
                    ))],
                ))],
            );
            if show.lit.is_some() {
                kids.push(C::J(empty));
            } else {
                let rest = self.cond("_rows.length > 0", C::J(body), None);
                kids.push(self.cond(&show.js, C::J(empty), Some(rest)));
            }
        }
        let a = self.base(n, true);
        block(
            vec![
                Code::of(vec![lit("const _cols = "), Seg::Code(columns), lit(";")]),
                Code::of(vec![lit("const _rows = "), Seg::Code(rows), lit(";")]),
            ],
            C::J(el("table", a, kids)),
        )
    }

    #[inline(never)]
    fn column(&mut self, n: &N<'a>) -> J<'a> {
        let mut a = self.base(n, true);
        a.push(attr_s("scope", "col"));
        let sort = self.text_of(n, "sort");
        match &sort.lit {
            Some(v) => {
                let v = v.string();
                if SORTS.contains(&v.as_str()) {
                    a.push(attr_s("aria-sort", v));
                }
            }
            None => a.push(attr_js(
                "aria-sort",
                format!(
                    "[\"none\", \"ascending\", \"descending\"].includes({s}) ? {s} : undefined",
                    s = sort.js
                ),
            )),
        }
        self.pressable(n, &mut a);
        let text = self.own_text(n);
        el("th", a, self.text_kids(text))
    }

    #[inline(never)]
    fn row(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> J<'a> {
        let mut a = self.base(n, true);
        if n.raw("selected").is_some() {
            let selected = self.flag_of(n, "selected");
            a.push(attr(
                "aria-selected",
                match &selected.lit {
                    Some(v) => AttrV::S(v.string()),
                    None => AttrV::Js(format!("String({})", selected.js)),
                },
            ));
        }
        self.pressable(n, &mut a);
        // Content that is not a cell is wrapped in one so the HTML parser keeps it in the row.
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let kids = self.kids(
            &pieces,
            ctx,
            Some(&mut |g: &mut Gen<'a>, leaf: Leaf<'_, 'a>| match leaf {
                Leaf::Text(text) => Some(C::J(el("td", vec![], vec![C::Text(text.to_owned())]))),
                Leaf::Node(p) if p.kind == "cell" => Some(C::J(g.cell(p, ctx))),
                Leaf::Node(p) => g.render(p, ctx).map(|c| C::J(el("td", vec![], vec![c]))),
            }),
        );
        el("tr", a, kids)
    }

    #[inline(never)]
    fn cell(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> J<'a> {
        let a = self.base(n, true);
        el("td", a, self.content(n, ctx))
    }

    /// SPEC §5.1: one `tabs` becomes a tablist of tab buttons followed by one tabpanel per tab,
    /// of which only the selected one is shown.
    #[inline(never)]
    fn tabs(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let children = self.pieces(n.children, &n.scope, n.depth + 1);
        let tabs = self.collect(&children, &mut |g, leaf| {
            let Leaf::Node(t) = leaf else {
                return None;
            };
            if t.kind != "tab" {
                return None;
            }
            let panel = el("", vec![], g.content(t, ctx));
            let state = g.text_of(t, "state");
            Some(Code::of(vec![
                lit(format!(
                    "{{ id: {}, doc: {}, state: {}, label: ",
                    str_js(&t.id),
                    quote(t.doc_id),
                    state.js
                )),
                Seg::Label(t.clone()),
                lit(", panel: "),
                expr(C::J(panel)),
                lit(" }"),
            ]))
        });
        let others = filter_pieces(&children, &|leaf| match leaf {
            Leaf::Text(_) => true,
            Leaf::Node(p) => p.kind != "tab",
        });
        let other = self.kids(&others, ctx, None);
        let write = self.write(n, "selected", "_x.doc");
        let change = self.fire(n, "change");
        let choose = Self::handler(
            if self.ts { "(_k: number)" } else { "(_k)" },
            vec![
                Some("const _x = _t[_k]".to_owned()),
                Some("if (!_x || _k === _s) return".to_owned()),
                write,
                change,
            ],
        );
        let selected = self.text_of(n, "selected");
        let busy = self.use_("_busy");
        let tab_key = self.use_("_tabKey");
        let key = |solid: bool| (!solid).then(|| attr_js("key", "_k"));
        let solid = self.solid();
        let mut button_attrs: Vec<Attr<'a>> = key(solid).into_iter().collect();
        button_attrs.extend([
            attr_js("data-weft-id", "_x.id || undefined"),
            attr_js("data-state", "_x.state || undefined"),
            attr_js("aria-busy", format!("{busy}(_x.state)")),
            attr_s("type", "button"),
            attr_s("role", "tab"),
            attr_js("id", "\"weft-\" + _x.id + \"-tab\""),
            attr_js("aria-selected", "String(_k === _s)"),
            attr_js("aria-controls", "\"weft-\" + _x.id + \"-panel\""),
            attr_js(self.an("tabIndex"), "_k === _s ? 0 : -1"),
            attr_js("onClick", "() => _choose(_k)"),
            attr_js(
                "onKeyDown",
                format!("(_e) => {tab_key}(_e, _k, _t.length, _choose)"),
            ),
        ]);
        let button = el("button", button_attrs, vec![C::Code(Code::lit("_x.label"))]);
        let mut panel_attrs: Vec<Attr<'a>> = key(solid).into_iter().collect();
        panel_attrs.extend([
            attr_s("role", "tabpanel"),
            attr_js("id", "\"weft-\" + _x.id + \"-panel\""),
            attr_js("aria-labelledby", "\"weft-\" + _x.id + \"-tab\""),
            attr_js("hidden", "_k !== _s"),
            attr_js(self.an("tabIndex"), "0"),
        ]);
        let panel = el("div", panel_attrs, vec![C::Code(Code::lit("_x.panel"))]);
        let mut list_attrs = self.base(n, true);
        list_attrs.push(attr_s("role", "tablist"));
        let mut list_kids = vec![C::Code(Code::of(vec![
            lit("_t.map((_x, _k) => "),
            expr(C::J(button)),
            lit(")"),
        ]))];
        list_kids.extend(other);
        let tablist = el("div", list_attrs, list_kids);
        block(
            vec![
                Code::of(vec![lit("const _t = "), Seg::Code(tabs), lit(";")]),
                Code::lit(format!(
                    "const _s = Math.max(0, _t.findIndex((_x) => _x.doc === {s} || _x.id === {s}));",
                    s = selected.js
                )),
                Code::lit(format!("const _choose = {choose};")),
            ],
            C::J(el(
                "div",
                vec![attr_s("data-weft-tabs", "")],
                vec![
                    C::J(tablist),
                    C::Code(Code::of(vec![
                        lit("_t.map((_x, _k) => "),
                        expr(C::J(panel)),
                        lit(")"),
                    ])),
                ],
            )),
        )
    }

    #[inline(never)]
    fn dialog(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        let open = self.flag_of(n, "open");
        if open.lit.as_ref().is_some_and(|v| *v != TRUE) {
            return None;
        }
        let mut a = self.base(n, true);
        a.push(attr("open", AttrV::True));
        let modal = if n.raw("modal").is_none() {
            lit_e(TRUE)
        } else {
            self.flag_of(n, "modal")
        };
        match &modal.lit {
            Some(v) => {
                if *v == TRUE {
                    a.push(attr_s("aria-modal", "true"));
                }
            }
            None => a.push(attr_js(
                "aria-modal",
                format!("{} ? \"true\" : undefined", modal.js),
            )),
        }
        let write = self.write(n, "open", "false");
        let close = self.fire(n, "close");
        a.push(attr_js(
            "onKeyDown",
            Self::handler(
                "(_e)",
                vec![
                    Some("if (_e.key !== \"Escape\") return".to_owned()),
                    Some("_e.preventDefault()".to_owned()),
                    write,
                    close,
                ],
            ),
        ));
        let dialog = C::J(el("dialog", a, self.content(n, ctx)));
        Some(match &open.lit {
            Some(_) => dialog,
            None if self.solid() => self.cond(&open.js, dialog, None),
            None => C::Code(Code::of(vec![
                lit(format!("{} && ", open.js)),
                expr(dialog),
            ])),
        })
    }

    // ---- The module ----

    fn root_body(&mut self, c: C<'a>, hidden: Option<String>) -> String {
        if self.solid() {
            // A component body runs once; only JSX tracks the data it reads, so an expression at
            // the root is put inside a fragment.
            let c = match hidden {
                Some(h) => self.cond(&format!("!{h}"), c, None),
                None => c,
            };
            let c = match c {
                C::Code(_) => C::J(el("", vec![], vec![c])),
                other => other,
            };
            return self.expr_of(&c, "  ");
        }
        match hidden {
            None => self.expr_of(&c, "  "),
            Some(h) => format!("{h} ? null : {}", self.expr_of(&c, "  ")),
        }
    }

    fn module(&self, name: &str, body: &str) -> String {
        let mut out = String::new();
        if self.solid() {
            if !self.solid.is_empty() {
                let names: Vec<&str> = self.solid.iter().copied().collect();
                out.push_str(&format!(
                    "import {{ {} }} from \"solid-js\";\n",
                    names.join(", ")
                ));
            }
            if !self.solid_web.is_empty() {
                let names: Vec<&str> = self.solid_web.iter().copied().collect();
                out.push_str(&format!(
                    "import {{ {} }} from \"solid-js/web\";\n",
                    names.join(", ")
                ));
            }
            if !out.is_empty() {
                out.push('\n');
            }
        } else if self.fragment {
            if self.ts && self.used.contains("_keyed") {
                out.push_str("import { Fragment, type ReactNode } from \"react\";\n\n");
            } else {
                out.push_str("import { Fragment } from \"react\";\n\n");
            }
        }
        if self.ts {
            out.push_str(TS_TYPES);
            if self.used.contains("_press") || self.used.contains("_tabKey") {
                out.push_str(TS_KEY);
            }
            out.push('\n');
        }
        for h in RUNTIME.iter().filter(|h| self.used.contains(h.name)) {
            out.push_str(if self.ts { h.ts } else { h.js });
            out.push_str("\n\n");
        }
        let params = match (self.solid(), self.ts) {
            (true, true) => "props: WeftProps",
            (true, false) => "props",
            (false, true) => "{ data, actions, onChange }: WeftProps",
            (false, false) => "{ data, actions, onChange }",
        };
        out.push_str(&format!(
            "export default function {name}({params}) {{\n  return {body};\n}}\n"
        ));
        out
    }
}

const TS_TYPES: &str = "export type WeftEvent = { id: string; action: string; item?: string };

export type WeftProps = {
  data?: any;
  actions?: Record<string, (event: WeftEvent) => void>;
  onChange?: (path: string, value: unknown) => void;
};
";

const TS_KEY: &str = "
type WeftKey = {
  key: string;
  target: unknown;
  currentTarget: { parentElement: HTMLElement | null };
  preventDefault(): void;
};
";

/// `(() => { const …; return <…>; })()`, for elements that need values computed once.
fn block<'a>(lines: Vec<Code<'a>>, result: C<'a>) -> C<'a> {
    C::Code(Code::of(vec![Seg::Block(lines, Box::new(result))]))
}

fn cell_wrap<'a>(c: Option<C<'a>>) -> Option<C<'a>> {
    c.map(|c| C::J(el("tr", vec![], vec![C::J(el("td", vec![], vec![c]))])))
}
