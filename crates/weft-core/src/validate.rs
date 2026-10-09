//! Schema and semantic layers of SPEC §6, plus the compatibility rules of §8. Validation reports;
//! it never panics and never changes the document.

use indexmap::IndexMap;
use serde_json::Value as Json;

use crate::diagnostics::{Code, Diagnostic, Mode, Position, did_you_mean, one_of, quote};
use crate::fragment::{FRAGMENT, OUTLET, PARAM, ParamKind, USE};
use crate::json::{JSON_DEPTH_LIMIT, js_number, js_round};
use crate::model::{
    Catalog, Child, ComponentDef, Content, Document, Map, Node, PropDef, PropType, Value,
    WEFT_VERSION,
};
use crate::rules::{
    ARIA_ROLES, EACH, MAX_DEPTH, MODEL_ASSETS, SLOT, asset_problem, embedded_reference,
    has_non_xml_char, is_action, is_binding, is_extension_name, is_id, is_loop_variable, is_name,
    is_token, universal_prop, version,
};
use crate::shape::{document_issues, to_document};
use crate::source::{NodeSource, path_segment};

mod fragments;
mod uses;

#[derive(Clone, Copy, Default)]
pub struct ValidateOptions<'a> {
    pub catalog: Option<&'a Catalog>,
    /// `Lenient` (default) warns about unknown content; `Strict` rejects it (SPEC §8).
    pub mode: Mode,
    /// Token path → DTCG `$type`. Token references are checked only when this is given.
    pub tokens: Option<&'a IndexMap<String, String>>,
    /// Known host actions. Action names are checked only when this is given.
    pub actions: Option<&'a [String]>,
}

const BINDING_GRAMMAR: &str = "$.name(.name)* or $loopVariable(.name)*";
const NAME_GRAMMAR: &str = "[a-z][a-z0-9]*(-[a-z0-9]+)*";

/// Validates JSON input: its shape first (W200), then the document it describes. Without a
/// catalog only the shape is checked.
pub fn validate(input: &Json, options: &ValidateOptions<'_>) -> Vec<Diagnostic> {
    let mode = options.mode;
    if json_exceeds_depth(input) {
        return vec![too_deep().mode(mode)];
    }
    let issues = document_issues(input);
    if !issues.is_empty() {
        return issues
            .iter()
            .map(|i| {
                Diagnostic::new(
                    Code::W200,
                    format!("#/{}", i.path_string()),
                    i.message.clone(),
                    "the canonical JSON shape of SPEC §3",
                )
                .mode(mode)
            })
            .collect();
    }
    validate_document(&to_document(input), options)
}

/// Validates a document built in memory or by `parse`; positions from `parse` are reported.
pub fn validate_document(document: &Document, options: &ValidateOptions<'_>) -> Vec<Diagnostic> {
    if model_exceeds_depth(&document.root) {
        return vec![too_deep().mode(options.mode)];
    }
    let Some(catalog) = options.catalog else {
        return vec![];
    };
    let mut v = Validator {
        catalog,
        options,
        out: vec![],
        ids: IndexMap::new(),
        references: vec![],
        in_fragment: false,
        params: Map::new(),
        outlets: vec![],
    };
    let root = &document.root;
    let root_path = format!("/{}", path_segment(&root.kind, root.id.as_deref(), None));
    let version_at = node_at(root, &root_path, Some("weft"));
    v.check_version(&document.weft, &version_at);
    if root.kind == FRAGMENT {
        v.visit_fragment(root, &root_path);
    } else {
        v.visit_node(root, &root_path, None, &[]);
    }

    for (value, target, at) in std::mem::take(&mut v.references) {
        if v.ids.get(&value).map(|(kind, _)| kind.as_str()) != Some(target.as_str()) {
            let ids: Vec<String> = v
                .ids
                .iter()
                .filter(|(_, (kind, _))| *kind == target)
                .map(|(id, _)| id.clone())
                .collect();
            let d = diag(
                Code::W309,
                &at,
                format!("\"{value}\" is not the id of a <{target}>."),
                one_of(&ids),
            )
            .got(value.clone())
            .hint_opt(did_you_mean(&value, &ids));
            v.report(d);
        }
    }
    v.out
}

fn too_deep() -> Diagnostic {
    Diagnostic::new(
        Code::W200,
        "#",
        format!("The document nests deeper than {MAX_DEPTH} elements."),
        format!("at most {MAX_DEPTH} levels"),
    )
}

/// Iterative, so that hostile JSON cannot overflow the stack before validation starts.
pub(crate) fn json_exceeds_depth(input: &Json) -> bool {
    let mut stack = vec![(input, 0usize)];
    while let Some((value, depth)) = stack.pop() {
        if depth > JSON_DEPTH_LIMIT {
            return true;
        }
        match value {
            Json::Array(items) => stack.extend(items.iter().map(|c| (c, depth + 1))),
            Json::Object(map) => stack.extend(map.values().map(|c| (c, depth + 1))),
            _ => {}
        }
    }
    false
}

/// The same bound as [`json_exceeds_depth`], measured on the JSON the document serializes to.
fn model_exceeds_depth(root: &Node) -> bool {
    // The root object sits one level below the document.
    let mut stack = vec![(root, 1usize)];
    while let Some((node, d)) = stack.pop() {
        let mut deepest = d + 1;
        if !node.props.is_empty() {
            let reference = node
                .props
                .values()
                .any(|v| matches!(v, Value::Bind { .. } | Value::Token(_)));
            deepest = deepest.max(if reference { d + 3 } else { d + 2 });
        }
        if !node.on.is_empty() {
            deepest = deepest.max(d + 2);
        }
        for list in node.slots.values() {
            deepest = deepest.max(d + 2);
            for child in list {
                deepest = deepest.max(d + 3);
                if let Child::Node(n) = child {
                    stack.push((n, d + 3));
                }
            }
        }
        for child in &node.children {
            deepest = deepest.max(d + 2);
            if let Child::Node(n) = child {
                stack.push((n, d + 2));
            }
        }
        if deepest > JSON_DEPTH_LIMIT {
            return true;
        }
    }
    false
}

#[derive(Clone)]
struct At {
    path: String,
    pos: Option<Position>,
}

fn source(node: &Node) -> Option<&NodeSource> {
    node.source.0.as_deref()
}

fn node_at(node: &Node, path: &str, attr: Option<&str>) -> At {
    let s = source(node);
    match attr {
        None => At {
            path: path.to_owned(),
            pos: s.map(|s| s.pos),
        },
        Some(attr) => At {
            path: format!("{path}/@{attr}"),
            pos: s
                .and_then(|s| s.attrs.get(attr).copied())
                .or(s.map(|s| s.pos)),
        },
    }
}

fn diag(
    code: Code,
    at: &At,
    message: impl Into<String>,
    expected: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(code, at.path.clone(), message, expected).pos(at.pos)
}

#[derive(Clone)]
struct Owner {
    /// Component kind the list belongs to; absent above the root.
    kind: Option<String>,
    content: Content,
    allowed: Option<Vec<String>>,
    /// Named slots hold elements only.
    slot: bool,
    /// Some ancestor is a `form`, which a `submit` button needs (SPEC §5.1).
    in_form: bool,
    /// The list is the body of an `<each>`, which repeats elements only (SPEC §4.3).
    each: bool,
}

impl Owner {
    fn new(kind: Option<String>, content: Content) -> Self {
        Owner {
            kind,
            content,
            allowed: None,
            slot: false,
            in_form: false,
            each: false,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Category {
    Each,
    /// A `<use>`; its definition is the fragment's signature when the fragment is known.
    Use,
    Component,
    Extension,
    Unknown,
}

struct Validator<'a> {
    catalog: &'a Catalog,
    options: &'a ValidateOptions<'a>,
    out: Vec<Diagnostic>,
    ids: IndexMap<String, (String, String)>,
    /// Literal values of props that name an element, with the kind they must name.
    references: Vec<(String, String, At)>,
    in_fragment: bool,
    /// The parameters of the fragment being checked; empty in a screen.
    params: Map<ParamKind>,
    /// Slot parameters an `<outlet>` has already placed.
    outlets: Vec<String>,
}

impl<'a> Validator<'a> {
    fn report(&mut self, d: Diagnostic) {
        self.out.push(d.mode(self.options.mode));
    }

    fn component(&self, kind: &str) -> Option<&'a ComponentDef> {
        let catalog: &'a Catalog = self.catalog;
        if kind == EACH {
            None
        } else {
            catalog.components.get(kind)
        }
    }

    /// What a node answers to: its component, or for a `<use>` its fragment's signature.
    fn def_of(&self, node: &Node) -> Option<&'a ComponentDef> {
        let catalog: &'a Catalog = self.catalog;
        if node.kind == EACH {
            None
        } else {
            catalog.def_of(node)
        }
    }

    fn check_value(
        &mut self,
        value: &Value,
        def: Option<&PropDef>,
        values: &[String],
        at: &At,
        scope: &[String],
    ) {
        match value {
            Value::Bind { bind, not } => return self.check_binding(bind, *not, def, at, scope),
            Value::Token(token) => return self.check_token(token, def, at),
            Value::String(s) => {
                if embedded_reference(s).is_some_and(|i| i > 0) {
                    self.report(
                        diag(
                            Code::W213,
                            at,
                            "A value is either text or one whole reference, never both.",
                            "plain text or one whole reference",
                        )
                        .got(s.clone())
                        .hint("bind the whole value, e.g. <text text=\"{$.greeting}\"/>"),
                    );
                }
                if has_non_xml_char(s) {
                    self.report(diag(
                        Code::W221,
                        at,
                        "The value contains a character XML cannot carry.",
                        "characters XML allows",
                    ));
                }
            }
            Value::Number(_) | Value::Bool(_) => {}
        }
        let Some(def) = def else { return };
        let got = quote(value);
        match def.kind {
            PropType::String => {
                if !matches!(value, Value::String(_)) {
                    let text = match value {
                        Value::Number(n) => js_number(*n),
                        Value::Bool(b) => b.to_string(),
                        _ => String::new(),
                    };
                    self.report(
                        diag(Code::W204, at, "Expected a string.", "string")
                            .got(got)
                            .hint(format!("write \"{text}\" as text")),
                    );
                }
            }
            PropType::Number => match value {
                Value::Number(n) if n.is_finite() => self.check_range(*n, def, at),
                _ => self
                    .report(diag(Code::W204, at, "Expected a number.", "a JSON number").got(got)),
            },
            PropType::Boolean => {
                if !matches!(value, Value::Bool(_)) {
                    self.report(
                        diag(Code::W204, at, "Expected a boolean.", "true or false").got(got),
                    );
                }
            }
            PropType::Enum => match value {
                Value::String(s) if !values.contains(s) => {
                    self.report(
                        diag(
                            Code::W203,
                            at,
                            format!("{got} is not an allowed value."),
                            one_of(values),
                        )
                        .got(got.clone())
                        .hint_opt(did_you_mean(s, values)),
                    );
                }
                Value::String(_) => {}
                _ => self.report(
                    diag(
                        Code::W204,
                        at,
                        "Expected one of the enum values.",
                        one_of(values),
                    )
                    .got(got),
                ),
            },
            PropType::Token => {
                let of_type = def
                    .token_type
                    .as_ref()
                    .map(|t| format!(" of type {t}"))
                    .unwrap_or_default();
                self.report(
                    diag(
                        Code::W204,
                        at,
                        "Expected a design token reference, not a raw value.",
                        format!("{{token.<path>}}{of_type}"),
                    )
                    .got(got)
                    .hint("use a token such as {token.space.md}"),
                );
            }
        }
    }

    fn check_range(&mut self, value: f64, def: &PropDef, at: &At) {
        let whole = def.integer == Some(true);
        let below = def.min.is_some_and(|min| value < min);
        let above = def.max.is_some_and(|max| value > max);
        if !below && !above && (!whole || value.fract() == 0.0) {
            return;
        }
        let range = match (def.min, def.max) {
            (Some(min), Some(max)) => format!(" from {} to {}", js_number(min), js_number(max)),
            (Some(min), None) => format!(" of at least {}", js_number(min)),
            (None, Some(max)) => format!(" of at most {}", js_number(max)),
            (None, None) => String::new(),
        };
        let expected = format!("{}{range}", if whole { "an integer" } else { "a number" });
        let mut nearest = if whole { js_round(value) } else { value };
        if let Some(min) = def.min {
            nearest = nearest.max(if whole { min.ceil() } else { min });
        }
        if let Some(max) = def.max {
            nearest = nearest.min(if whole { max.floor() } else { max });
        }
        let shown = js_number(value);
        self.report(
            diag(
                Code::W224,
                at,
                format!("{shown} is not {expected}."),
                expected,
            )
            .got(shown)
            .hint(format!("use {}", js_number(nearest))),
        );
    }

    fn check_binding(
        &mut self,
        path: &str,
        not: bool,
        def: Option<&PropDef>,
        at: &At,
        scope: &[String],
    ) {
        if !is_binding(path) {
            self.report(
                diag(
                    Code::W214,
                    at,
                    "The binding path is malformed.",
                    BINDING_GRAMMAR,
                )
                .got(path),
            );
        } else if !path.starts_with("$.") {
            let variable = path[1..].split('.').next().unwrap_or_default();
            if self.params.contains_key(variable) {
                if !self.check_param_read(path, variable, not, def, at) {
                    return;
                }
            } else if !scope.iter().any(|s| s == variable) {
                let expected = if scope.is_empty() {
                    "a path starting with $.".to_owned()
                } else {
                    format!("$.… or {}", one_of(scope.iter().map(|v| format!("${v}"))))
                };
                self.report(
                    diag(
                        Code::W305,
                        at,
                        format!(
                            "Loop variable \"{variable}\" is not defined by an enclosing <each>."
                        ),
                        expected,
                    )
                    .got(path)
                    .hint_opt(did_you_mean(variable, scope)),
                );
            }
        }
        let Some(def) = def else { return };
        let bang = if not { "!" } else { "" };
        if def.bindable == Some(false) {
            self.report(
                diag(
                    Code::W217,
                    at,
                    "This attribute takes a literal only.",
                    "a literal",
                )
                .got(format!("{{{bang}{path}}}")),
            );
        } else if not && def.kind != PropType::Boolean {
            self.report(
                diag(
                    Code::W218,
                    at,
                    "Only boolean attributes take a negated binding.",
                    "a plain binding",
                )
                .got(format!("{{!{path}}}"))
                .hint(format!("bind {{{path}}} instead")),
            );
        } else if not && def.writable == Some(true) {
            self.report(
                diag(
                    Code::W218,
                    at,
                    "A two-way attribute cannot take a read-only negated binding.",
                    "a plain binding",
                )
                .got(format!("{{!{path}}}"))
                .hint(format!("bind {{{path}}} instead")),
            );
        }
    }

    fn check_token(&mut self, path: &str, def: Option<&PropDef>, at: &At) {
        if let Some(def) = def.filter(|d| d.kind != PropType::Token) {
            let ty = def.kind.as_str();
            self.report(
                diag(
                    Code::W204,
                    at,
                    format!("Expected a {ty}, not a token reference."),
                    ty,
                )
                .got(format!("{{token.{path}}}")),
            );
        }
        if !is_token(path) {
            self.report(
                diag(
                    Code::W215,
                    at,
                    "The token path is malformed.",
                    "segments of [A-Za-z0-9_-] joined by dots",
                )
                .got(path),
            );
            return;
        }
        let Some(tokens) = self.options.tokens else {
            return;
        };
        match tokens.get(path) {
            None => self.report(
                diag(
                    Code::W306,
                    at,
                    format!("Token \"{path}\" does not exist."),
                    "a token from the token set",
                )
                .got(path)
                .hint_opt(did_you_mean(path, tokens.keys())),
            ),
            Some(ty) => {
                if let Some(wanted) = def.and_then(|d| d.token_type.as_ref()).filter(|w| *w != ty) {
                    let same: Vec<&String> = tokens
                        .iter()
                        .filter(|(_, t)| *t == wanted)
                        .map(|(p, _)| p)
                        .collect();
                    self.report(
                        diag(
                            Code::W307,
                            at,
                            format!("Token \"{path}\" has type {ty}."),
                            wanted.clone(),
                        )
                        .got(ty.clone())
                        .hint_opt(did_you_mean(path, same)),
                    );
                }
            }
        }
    }

    fn check_role(&mut self, value: &Value, at: &At) {
        match value {
            Value::String(role) if !ARIA_ROLES.contains(&role.as_str()) => self.report(
                diag(
                    Code::W211,
                    at,
                    format!("\"{role}\" is not a WAI-ARIA role."),
                    "a WAI-ARIA 1.2 role",
                )
                .got(role.clone())
                .hint_opt(did_you_mean(role, ARIA_ROLES)),
            ),
            Value::String(_) => {}
            _ => self.report(
                diag(
                    Code::W217,
                    at,
                    "`role` takes a literal only.",
                    "a literal ARIA role",
                )
                .got(quote(value)),
            ),
        }
    }

    /// Whether `grow` has a stack to grow in (SPEC §2.2). `<each>` is transparent, and an unknown
    /// or extension parent is opaque (SPEC §8): it may be a newer container that grows children.
    fn grows_in_stack(&self, owner: Option<&Owner>) -> bool {
        owner.is_some_and(|o| {
            !o.slot
                && o.kind
                    .as_ref()
                    .is_some_and(|k| k == "stack" || !self.catalog.components.contains_key(k))
        })
    }

    fn visit_list(
        &mut self,
        list: &[Child],
        list_path: &str,
        owner: &Owner,
        scope: &[String],
        positions: Option<&[Option<Position>]>,
    ) {
        let where_ = if owner.each {
            "<each>".to_owned()
        } else if owner.slot {
            "This slot".to_owned()
        } else {
            owner
                .kind
                .as_ref()
                .map_or_else(|| "This list".to_owned(), |k| format!("<{k}>"))
        };
        for (index, child) in list.iter().enumerate() {
            let child = match child {
                Child::Text(text) => {
                    let at = At {
                        path: format!("{list_path}/#text[{index}]"),
                        pos: positions.and_then(|p| p.get(index).copied().flatten()),
                    };
                    if owner.slot
                        || owner.each
                        || matches!(owner.content, Content::Nodes | Content::None)
                    {
                        let expected = if owner.content == Content::None {
                            "no content"
                        } else {
                            "elements"
                        };
                        self.report(
                            diag(
                                Code::W304,
                                &at,
                                format!("{where_} does not take text."),
                                expected,
                            )
                            .got(quote(text))
                            .hint("wrap the text in <text id=\"…\">"),
                        );
                    }
                    if has_non_xml_char(text) {
                        self.report(diag(
                            Code::W221,
                            &at,
                            "The text contains a character XML cannot carry.",
                            "characters XML allows",
                        ));
                    }
                    continue;
                }
                Child::Node(n) => n,
            };
            let child_path = format!(
                "{list_path}/{}",
                path_segment(&child.kind, child.id.as_deref(), Some(index))
            );
            let at = node_at(child, &child_path, None);
            if child.kind == PARAM
                && owner.kind.as_deref() == Some(FRAGMENT)
                && !owner.slot
                && !owner.each
            {
                // Declarations, checked with the fragment itself.
                continue;
            }
            let component = self.def_of(child);
            let opaque = child.kind != EACH && component.is_none();
            if child.kind == USE
                && component.is_some()
                && !matches!(owner.content, Content::None | Content::Text)
            {
                // A use is transparent: what it places answers to this list (SPEC §10.7).
                self.check_placed(child, &at, owner, &where_);
                self.visit_node(child, &child_path, Some(owner), scope);
                continue;
            }
            if matches!(owner.content, Content::None | Content::Text) {
                let expected = if owner.content == Content::None {
                    "no content"
                } else {
                    "text"
                };
                self.report(
                    diag(
                        Code::W304,
                        &at,
                        format!("{where_} does not take elements."),
                        expected,
                    )
                    .got(format!("<{}>", child.kind)),
                );
            } else if let Some(allowed) = owner
                .allowed
                .as_ref()
                .filter(|a| !opaque && !a.contains(&child.kind))
            {
                // Unknown and extension elements are opaque (SPEC §8), so containment rules skip them.
                self.report(
                    diag(
                        Code::W302,
                        &at,
                        format!("{where_} does not take <{}>.", child.kind),
                        one_of(allowed),
                    )
                    .got(child.kind.clone()),
                );
            }
            if let (Some(parents), Some(owner_kind)) = (
                component.and_then(|c| c.allowed_parents.as_ref()),
                owner.kind.as_ref(),
            ) {
                // An unknown or extension parent is opaque (SPEC §8): it may wrap a newer container.
                if self.catalog.components.contains_key(owner_kind) && !parents.contains(owner_kind)
                {
                    self.report(
                        diag(
                            Code::W303,
                            &at,
                            format!("<{}> cannot be placed in <{owner_kind}>.", child.kind),
                            format!("a parent {}", one_of(parents)),
                        )
                        .got(owner_kind.clone()),
                    );
                }
            }
            self.visit_node(child, &child_path, Some(owner), scope);
        }
    }

    fn visit_node(&mut self, node: &Node, path: &str, owner: Option<&Owner>, scope: &[String]) {
        let kind = node.kind.as_str();
        let at = node_at(node, path, None);
        let props = &node.props;
        let catalog = self.catalog;
        if owner.is_some() && (kind == OUTLET || kind == PARAM) {
            return self.visit_structural(node, path);
        }
        let component = self.def_of(node);
        let category = if kind == EACH {
            Category::Each
        } else if kind == USE {
            Category::Use
        } else if component.is_some() {
            Category::Component
        } else if kind.starts_with("x-") {
            Category::Extension
        } else {
            Category::Unknown
        };
        let kinds = || catalog.components.keys();

        if category == Category::Use && component.is_none() {
            self.unknown_fragment(node, path);
        } else if category == Category::Unknown {
            if kind == SLOT || !is_name(kind) {
                let (message, hint) = if kind == SLOT {
                    (
                        "`slot` is structural; named slots live in `slots`.".to_owned(),
                        None,
                    )
                } else {
                    (
                        format!("Kind \"{kind}\" breaks the name grammar."),
                        did_you_mean(kind, kinds()),
                    )
                };
                self.report(
                    diag(
                        Code::W223,
                        &at,
                        message,
                        "a catalog kind or x-<vendor>-<name>",
                    )
                    .got(kind)
                    .hint_opt(hint),
                );
            } else {
                let hint = did_you_mean(kind, kinds()).unwrap_or_else(|| {
                    format!("use a catalog component, or an extension named x-<vendor>-{kind}")
                });
                self.report(
                    diag(
                        Code::W401,
                        &at,
                        format!(
                            "<{kind}> is not in catalog {} {}.",
                            catalog.name, catalog.version
                        ),
                        "a catalog component or an x-<vendor>- extension",
                    )
                    .got(kind)
                    .hint(hint),
                );
            }
        } else if category == Category::Extension && !is_extension_name(kind) {
            self.report(
                diag(
                    Code::W220,
                    &at,
                    format!("Extension <{kind}> needs a vendor prefix."),
                    "x-<vendor>-<name>",
                )
                .got(kind),
            );
        }
        let is_root_kind = |kind: &str| {
            catalog
                .components
                .get(kind)
                .is_some_and(|c| c.root == Some(true))
        };
        if owner.is_none() {
            let roots: Vec<&str> = kinds()
                .filter(|k| is_root_kind(k))
                .map(String::as_str)
                .collect();
            if !roots.is_empty() && !roots.contains(&kind) {
                self.report(
                    diag(
                        Code::W201,
                        &at,
                        format!("The root element must be <{}>.", roots.join("> or <")),
                        roots.join(" or "),
                    )
                    .got(kind),
                );
            }
        } else if is_root_kind(kind) {
            self.report(
                diag(
                    Code::W312,
                    &at,
                    format!("<{kind}> is allowed only as the root."),
                    "a container below the root",
                )
                .hint("use <section> or <stack>"),
            );
        }

        match &node.id {
            None => self.report(
                diag(Code::W202, &at, format!("<{kind}> has no id."), "id=\"…\"")
                    .hint("add a document-unique id"),
            ),
            Some(id) if !is_id(id) => self.report(
                diag(
                    Code::W212,
                    &node_at(node, path, Some("id")),
                    format!("Id {} breaks the id grammar.", quote(id)),
                    "[A-Za-z][A-Za-z0-9_-]*",
                )
                .got(id.clone()),
            ),
            Some(id) => match self.ids.get(id) {
                None => {
                    self.ids
                        .insert(id.clone(), (kind.to_owned(), path.to_owned()));
                }
                Some((_, first)) => {
                    let hint = format!("first used at {first}; pick another id");
                    self.report(
                        diag(
                            Code::W301,
                            &node_at(node, path, Some("id")),
                            format!("Id \"{id}\" is already used."),
                            "a document-unique id",
                        )
                        .got(id.clone())
                        .hint(hint),
                    );
                }
            },
        }

        for (name, value) in props {
            let attr_at = node_at(node, path, Some(name));
            if !is_name(name) || name == "id" || name.starts_with("on-") {
                let message = if name == "id" {
                    "`id` belongs in Node.id, not in props.".to_owned()
                } else if name.starts_with("on-") {
                    "Events belong in `on`, not in props.".to_owned()
                } else {
                    format!("Attribute name \"{name}\" breaks the name grammar.")
                };
                self.report(
                    diag(
                        Code::W223,
                        &attr_at,
                        message,
                        "a prop name matching [a-z][a-z0-9]*(-[a-z0-9]+)*",
                    )
                    .got(name.clone()),
                );
                continue;
            }
            if owner.is_none() && name == "weft" {
                self.report(diag(
                    Code::W200,
                    &attr_at,
                    "The format version lives in Document.weft, not in the root's props.",
                    "Document.weft",
                ));
                continue;
            }
            if let Some(component) = component.filter(|_| category == Category::Use) {
                let def = component.prop(name);
                if def.is_none() && !name.starts_with("x-") {
                    self.undeclared(node, &attr_at, name, "attribute");
                }
                let values = def.and_then(|d| d.values.clone()).unwrap_or_default();
                self.check_value(value, def, &values, &attr_at, scope);
                continue;
            }
            if let Some(component) = component {
                if name == "role" {
                    self.report(
                        diag(
                            Code::W209,
                            &attr_at,
                            format!("<{kind}> already has a role from the catalog."),
                            "no role attribute",
                        )
                        .hint("remove role"),
                    );
                    continue;
                }
                let Some(def) = component.prop(name).or_else(|| universal_prop(name)) else {
                    if !name.starts_with("x-") {
                        let mut known: Vec<String> = component
                            .props
                            .iter()
                            .flat_map(|p| p.keys().cloned())
                            .collect();
                        for universal in ["label", "hidden", "state"] {
                            if !known.iter().any(|k| k == universal) {
                                known.push(universal.to_owned());
                            }
                        }
                        self.report(
                            diag(
                                Code::W402,
                                &attr_at,
                                format!("<{kind}> has no attribute \"{name}\"."),
                                one_of(&known),
                            )
                            .got(name.clone())
                            .hint_opt(did_you_mean(name, &known)),
                        );
                    }
                    self.check_value(value, None, &[], &attr_at, scope);
                    continue;
                };
                let values = if name == "state" && component.prop(name).is_none() {
                    component.states.clone().unwrap_or_default()
                } else {
                    def.values.clone().unwrap_or_default()
                };
                self.check_value(value, Some(def), &values, &attr_at, scope);
                continue;
            }
            if category == Category::Each && name != "in" && name != "as" && !name.starts_with("x-")
            {
                self.report(
                    diag(
                        Code::W402,
                        &attr_at,
                        format!("<each> has no attribute \"{name}\"."),
                        one_of(["in", "as"]),
                    )
                    .got(name.clone())
                    .hint_opt(did_you_mean(name, ["in", "as"])),
                );
            }
            if category != Category::Each && name == "role" {
                self.check_role(value, &attr_at);
            } else {
                self.check_value(value, None, &[], &attr_at, scope);
            }
        }

        if !matches!(category, Category::Each | Category::Use)
            && props.get("grow") == Some(&Value::Bool(true))
            && !self.grows_in_stack(owner)
        {
            self.report(
                diag(
                    Code::W318,
                    &node_at(node, path, Some("grow")),
                    format!("<{kind}> grows, but its parent is not a <stack>."),
                    "a parent <stack>",
                )
                .hint("move the element into a <stack>, or remove grow"),
            );
        }

        let mut inner_scope = scope.to_vec();
        if let Some(component) = component {
            for (name, def) in component.props.iter().flatten() {
                if def.required != Some(true)
                    || props.contains_key(name)
                    || (owner.is_none() && name == "weft")
                {
                    continue;
                }
                self.report(diag(
                    Code::W205,
                    &at,
                    format!("<{kind}> needs \"{name}\"."),
                    format!("{name}=\"…\""),
                ));
            }
            if component.requires_label == Some(true)
                && !props.contains_key("label")
                && component.prop("label").and_then(|d| d.required) != Some(true)
            {
                self.report(diag(
                    Code::W205,
                    &at,
                    format!("<{kind}> needs an accessible name."),
                    "label=\"…\"",
                ));
            }
            if matches!(component.content, Content::Text | Content::Mixed)
                && props.contains_key("text")
                && !node.children.is_empty()
            {
                // SPEC §5.1 note; the catalog format cannot express "content or the text prop".
                self.report(
                    diag(Code::W310, &node_at(node, path, Some("text")), format!("<{kind}> takes its text from content or from the text attribute, not both."), "content or text, not both")
                        .hint("remove the content or the text attribute"),
                );
            }
            if props.get("submit") == Some(&Value::Bool(true))
                && category != Category::Use
                && !owner.is_some_and(|o| o.in_form)
            {
                self.report(
                    diag(
                        Code::W313,
                        &node_at(node, path, Some("submit")),
                        format!("<{kind}> submits a form but has no enclosing <form>."),
                        "an enclosing <form>",
                    )
                    .hint("move it into a <form>, or remove submit=\"true\" and give it on-press"),
                );
            }
            // The asset paths of a `model` are untrusted, and the catalog format has no path type.
            if kind == "model" {
                for (name, extensions) in MODEL_ASSETS {
                    let Some(Value::String(value)) = props.get(name) else {
                        continue;
                    };
                    let Some(problem) = asset_problem(value, extensions) else {
                        continue;
                    };
                    let end = value.floor_char_boundary(80);
                    self.report(
                        diag(
                            Code::W317,
                            &node_at(node, path, Some(name)),
                            format!("<model> {name} is not an asset path: {problem}."),
                            format!(
                                "a path relative to the project, or an https URL, ending in {}",
                                extensions.join(" or ")
                            ),
                        )
                        .got(value[..end].to_owned())
                        .hint("write a relative path such as assets/chair.glb"),
                    );
                }
            }
            for (name, def) in component.props.iter().flatten() {
                if let (Some(target), Some(Value::String(id))) = (&def.references, props.get(name))
                {
                    self.references.push((
                        id.clone(),
                        target.clone(),
                        node_at(node, path, Some(name)),
                    ));
                }
            }
        } else if category == Category::Extension && !props.contains_key("role") {
            self.report(
                diag(
                    Code::W210,
                    &at,
                    format!("Extension <{kind}> needs a role as its fallback."),
                    "role=\"…\"",
                )
                .hint("add role=\"group\""),
            );
        } else if category == Category::Each {
            let items = props.get("in");
            if !matches!(items, Some(Value::Bind { not: false, .. })) {
                self.report(
                    diag(
                        Code::W222,
                        &node_at(node, path, Some("in")),
                        "<each> needs `in` bound to an array.",
                        "in=\"{$.items}\"",
                    )
                    .got_opt(items.map(quote)),
                );
            }
            let variable = props.get("as");
            match variable {
                Some(Value::String(v)) if is_loop_variable(v) => {
                    if scope.contains(v) {
                        self.report(
                            diag(
                                Code::W311,
                                &node_at(node, path, Some("as")),
                                format!(
                                    "Loop variable \"{v}\" is already defined by an outer <each>."
                                ),
                                format!("a name other than {}", one_of(scope)),
                            )
                            .got(v.clone())
                            .hint("pick another name"),
                        );
                    } else {
                        inner_scope.push(v.clone());
                    }
                }
                _ => self.report(
                    diag(
                        Code::W222,
                        &node_at(node, path, Some("as")),
                        "<each> needs `as` naming the loop variable.",
                        "[a-z][A-Za-z0-9]*",
                    )
                    .got_opt(variable.map(quote)),
                ),
            }
            if !node.children.iter().any(|c| matches!(c, Child::Node(_))) {
                self.report(
                    diag(
                        Code::W314,
                        &at,
                        "<each> has no element to repeat.",
                        "one or more child elements",
                    )
                    .hint("put the element to repeat inside <each>, or remove it"),
                );
            }
        }

        let events: Vec<String> = component.and_then(|c| c.events.clone()).unwrap_or_default();
        for (event, action) in &node.on {
            let event_at = node_at(node, path, Some(&format!("on-{event}")));
            if !is_name(event) {
                self.report(
                    diag(
                        Code::W223,
                        &event_at,
                        format!("Event name \"{event}\" breaks the name grammar."),
                        "[a-z0-9]+(-[a-z0-9]+)*",
                    )
                    .got(event.clone()),
                );
            } else if category == Category::Use && component.is_some() && !events.contains(event) {
                self.undeclared(node, &event_at, event, "action");
            } else if matches!(category, Category::Component | Category::Each)
                && !events.contains(event)
            {
                self.report(
                    diag(
                        Code::W206,
                        &event_at,
                        format!("<{kind}> has no event \"{event}\"."),
                        one_of(&events),
                    )
                    .got(event.clone())
                    .hint_opt(did_you_mean(event, &events)),
                );
            }
            if self.check_action_read(action, &event_at) {
            } else if !is_action(action) {
                self.report(
                    diag(
                        Code::W216,
                        &event_at,
                        "The action name is malformed.",
                        "[a-z][A-Za-z0-9]*(.[a-z][A-Za-z0-9]*)*",
                    )
                    .got(action.clone()),
                );
            } else if let Some(actions) = self.options.actions.filter(|a| !a.contains(action)) {
                self.report(
                    diag(
                        Code::W308,
                        &event_at,
                        format!("Action \"{action}\" is not provided by the host."),
                        one_of(actions),
                    )
                    .got(action.clone())
                    .hint_opt(did_you_mean(action, actions)),
                );
            }
        }

        let in_form = owner.is_some_and(|o| o.in_form) || kind == "form";
        let slot_sources = source(node).map(|s| &s.slots);
        for (name, list) in &node.slots {
            let slot_path = format!("{path}/slot[{name}]");
            let slot_source = slot_sources.and_then(|s| s.get(name));
            let slot_at = At {
                path: slot_path.clone(),
                pos: slot_source.map(|s| s.pos).or(at.pos),
            };
            let declared = component.and_then(|c| c.slot(name));
            if !is_name(name) {
                self.report(
                    diag(
                        Code::W223,
                        &slot_at,
                        format!("Slot name \"{name}\" breaks the name grammar."),
                        NAME_GRAMMAR,
                    )
                    .got(name.clone()),
                );
            } else if category == Category::Use && component.is_some() && declared.is_none() {
                self.undeclared(node, &slot_at, name, "slot");
            } else if matches!(category, Category::Component | Category::Each) && declared.is_none()
            {
                let names: Vec<String> = component
                    .iter()
                    .flat_map(|c| c.slots.iter().flat_map(|s| s.keys().cloned()))
                    .collect();
                self.report(
                    diag(
                        Code::W207,
                        &slot_at,
                        format!("<{kind}> has no slot \"{name}\"."),
                        one_of(&names),
                    )
                    .got(name.clone())
                    .hint_opt(did_you_mean(name, &names)),
                );
            }
            let slot_owner = Owner {
                kind: if category == Category::Each {
                    owner.and_then(|o| o.kind.clone())
                } else {
                    Some(kind.to_owned())
                },
                content: Content::Nodes,
                allowed: declared.and_then(|d| d.allowed_children.clone()),
                slot: true,
                in_form,
                each: false,
            };
            let positions = slot_source.map(|s| s.children.as_slice());
            self.visit_list(list, &slot_path, &slot_owner, &inner_scope, positions);
        }
        if let Some(component) = component {
            for (name, def) in component.slots.iter().flatten() {
                if def.required == Some(true) && !node.slots.contains_key(name) {
                    self.report(diag(
                        Code::W208,
                        &at,
                        format!("<{kind}> needs slot \"{name}\"."),
                        format!("<slot name=\"{name}\">"),
                    ));
                }
            }
        }

        // `<each>` is transparent: its children answer to the list that holds the `<each>` (SPEC §4.3).
        let child_owner = match (component, category) {
            (Some(component), _) => Owner {
                allowed: component.allowed_children.clone(),
                in_form,
                ..Owner::new(Some(kind.to_owned()), component.content)
            },
            (None, Category::Each) => Owner {
                each: true,
                ..owner
                    .cloned()
                    .unwrap_or_else(|| Owner::new(None, Content::Mixed))
            },
            (None, _) => Owner {
                in_form,
                ..Owner::new(Some(kind.to_owned()), Content::Mixed)
            },
        };
        let positions = source(node).map(|s| s.children.as_slice());
        self.visit_list(&node.children, path, &child_owner, &inner_scope, positions);
    }

    fn check_version(&mut self, weft: &str, at: &At) {
        let fix = format!("weft=\"{WEFT_VERSION}\"");
        let (current_major, current_minor) = version(WEFT_VERSION).unwrap_or(("0", "0"));
        let number = |s: &str| s.parse::<f64>().unwrap_or(f64::INFINITY);
        if weft.is_empty() {
            self.report(diag(
                Code::W205,
                at,
                "The root needs the format version.",
                fix,
            ));
        } else if let Some((major, minor)) = version(weft) {
            if number(major) != number(current_major) {
                self.report(
                    diag(
                        Code::W404,
                        at,
                        format!("Version {weft} is not readable by a {WEFT_VERSION} reader."),
                        format!("{current_major}.x"),
                    )
                    .got(weft),
                );
            } else if number(minor) > number(current_minor) {
                self.report(
                    diag(Code::W403, at, format!("Version {weft} is newer than {WEFT_VERSION}; unknown content is read as extensions."), format!("at most {WEFT_VERSION}"))
                        .got(weft),
                );
            }
        } else {
            self.report(
                diag(
                    Code::W219,
                    at,
                    "The format version must be major.minor.",
                    "major.minor",
                )
                .got(weft)
                .hint(format!("write {fix}")),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_round_rounds_halves_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.499_999_999_999_999_94), 0.0);
    }

    #[test]
    fn hostile_json_depth_is_reported_not_walked() {
        let mut v = Json::Null;
        for _ in 0..JSON_DEPTH_LIMIT + 2 {
            v = Json::Array(vec![v]);
        }
        let d = validate(&v, &ValidateOptions::default());
        assert_eq!(d.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::W200]);
    }

    #[test]
    fn shape_errors_point_into_the_json() {
        let v = serde_json::json!({"weft": "0.1", "root": {"kind": "screen", "id": "s", "children": [{"id": 1}]}});
        let d = validate(&v, &ValidateOptions::default());
        assert_eq!(
            d.iter().map(|d| d.path.as_str()).collect::<Vec<_>>(),
            ["#/root/children/0"]
        );
    }
}
