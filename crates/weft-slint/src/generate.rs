//! Weft → Slint: one exported component per screen, built from the std widgets, with the
//! canonical markup in a leading `// weft:source slint` comment so `import_slint` gives the
//! document back. Only document text that went through `names::string` reaches the output.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;
use weft_catalog::{Token, token_types};
use weft_core::{
    Catalog, Child, Diagnostic, Document, Mode, Node, ValidateOptions, Value, has_errors,
    js_number, serialize, validate_document,
};

use crate::Unsupported;
use crate::data::{Data, Ty, prop_type};
use crate::names::{clash_key, identifier, string, type_name};

/// The first line of the source comment; `import_slint` looks for it.
pub(crate) const MARKER: &str = "// weft:source slint";
const INDENT: &str = "    ";
/// CSS `rem` against the browser default; Slint `px` are logical pixels like CSS ones.
const PX_PER_REM: f64 = 16.0;
/// Font sizes of heading levels 1 to 6, in px, as the default browser stylesheet gives them.
const HEADING_FONT_PX: [u32; 6] = [32, 24, 19, 16, 13, 11];
const HEADING_WEIGHT: u32 = 700;

pub struct GenerateOptions<'a> {
    pub catalog: &'a Catalog,
    /// Resolved design tokens (`weft_catalog::load_tokens`); a `gap` token becomes its px value.
    pub tokens: &'a IndexMap<String, Token>,
    /// The component is named `<Name>Screen`; the screen id when absent.
    pub name: Option<&'a str>,
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    /// The document does not validate in strict mode against the catalog and tokens.
    #[error("the document has {} diagnostics", .0.len())]
    Invalid(Vec<Diagnostic>),
    /// The document is valid, but says something the generator cannot express in Slint.
    #[error("cannot generate Slint: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Unsupported(Vec<Unsupported>),
}

/// The `.slint` source of `document`: an exported `Window` component with one `in-out property`
/// per data path, a `perform(action, id)` callback, and the screen's elements.
pub fn generate(
    document: &Document,
    options: &GenerateOptions<'_>,
) -> Result<String, GenerateError> {
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
        return Err(GenerateError::Invalid(diagnostics));
    }
    let mut problems = vec![];
    check_ids(&document.root, &mut HashMap::new(), &mut problems);
    let data = Data::infer(&document.root, options.catalog, &mut problems);
    let mut g = Gen {
        catalog: options.catalog,
        tokens: options.tokens,
        data,
        out: String::new(),
        imports: BTreeSet::new(),
        forms: vec![],
        problems,
    };
    let name = options
        .name
        .or(document.root.id.as_deref())
        .unwrap_or("screen");
    g.screen(&document.root, &format!("{}Screen", type_name(name)));
    if !g.problems.is_empty() {
        return Err(GenerateError::Unsupported(g.problems));
    }
    let mut head = match options.name {
        Some(n) => format!("{MARKER} name={n}\n"),
        None => format!("{MARKER}\n"),
    };
    for line in serialize(document).lines() {
        head.push_str(&format!("// {line}\n"));
    }
    if !g.imports.is_empty() {
        let list: Vec<&str> = g.imports.iter().copied().collect();
        head.push_str(&format!(
            "\nimport {{ {} }} from \"std-widgets.slint\";\n",
            list.join(", ")
        ));
    }
    Ok(format!("{head}\n{}", g.out))
}

fn path_of(node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{}#{id}", node.kind),
        None => node.kind.clone(),
    }
}

fn element_children(node: &Node) -> impl Iterator<Item = &Node> {
    node.children.iter().filter_map(Child::as_node)
}

/// Every id must be a Slint element id, and no two may be the same id to Slint.
fn check_ids(node: &Node, seen: &mut HashMap<String, String>, problems: &mut Vec<Unsupported>) {
    if let Some(id) = &node.id {
        if identifier(id).is_none() {
            problems.push(Unsupported::new(
                path_of(node),
                "the id is reserved in Slint",
            ));
        } else if let Some(other) = seen.insert(clash_key(id), id.clone()) {
            problems.push(Unsupported::new(
                path_of(node),
                format!("Slint reads this id and `{other}` as the same id"),
            ));
        }
    }
    let slots = node.slots.values().flatten();
    for child in node.children.iter().chain(slots) {
        if let Child::Node(child) = child {
            check_ids(child, seen, problems);
        }
    }
}

struct Gen<'a> {
    catalog: &'a Catalog,
    tokens: &'a IndexMap<String, Token>,
    data: Data,
    out: String,
    imports: BTreeSet<&'static str>,
    /// The enclosing forms' submit calls, innermost last; `None` for a form without `on-submit`.
    forms: Vec<Option<String>>,
    problems: Vec<Unsupported>,
}

impl Gen<'_> {
    fn line(&mut self, depth: usize, text: &str) {
        self.out.push_str(&INDENT.repeat(depth));
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn unsupported(&mut self, node: &Node, message: &str) {
        self.problems.push(Unsupported::new(path_of(node), message));
    }

    /// A prop value as a Slint expression of type `want`.
    fn value(&mut self, node: &Node, prop: &str, want: Ty) -> Option<String> {
        let expr = match node.props.get(prop)? {
            Value::String(s) if want == Ty::Str => Some(string(s)),
            Value::Number(n) if want != Ty::Bool && want != Ty::Str => Some(js_number(*n)),
            Value::Bool(b) if want == Ty::Bool => Some(b.to_string()),
            Value::Bind { bind, not } => self.data.read(bind, *not, want),
            _ => None,
        };
        if expr.is_none() {
            self.unsupported(
                node,
                &format!("`{prop}` has no Slint form of type {}", want.slint()),
            );
        }
        expr
    }

    /// `prop: value;` or, for a writable binding, `prop <=> root.path;`.
    fn bind_line(&mut self, depth: usize, node: &Node, prop: &str, slint: &str, want: Ty) {
        if let Some(Value::Bind { bind, not }) = node.props.get(prop) {
            let (_, writable) = prop_type(self.catalog, &node.kind, prop);
            if let Some(target) = writable
                .then(|| self.data.two_way(bind, *not, want))
                .flatten()
            {
                return self.line(depth, &format!("{slint} <=> {target};"));
            }
        }
        if node.props.contains_key(prop)
            && let Some(v) = self.value(node, prop, want)
        {
            self.line(depth, &format!("{slint}: {v};"));
        }
    }

    /// The text a `text`-content kind shows: its content, or its `text` prop.
    fn text(&mut self, node: &Node) -> String {
        if node.props.contains_key("text") {
            return self.value(node, "text", Ty::Str).unwrap_or_default();
        }
        let parts: Vec<&str> = node
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect();
        string(&parts.join(" "))
    }

    fn call(node: &Node, event: &str) -> Option<String> {
        let action = node.on.get(event)?;
        let id = node.id.as_deref().unwrap_or_default();
        Some(format!("root.perform({}, {});", string(action), string(id)))
    }

    fn handler(&mut self, depth: usize, callback: &str, calls: &[Option<String>]) {
        let body: Vec<&str> = calls.iter().flatten().map(String::as_str).collect();
        if !body.is_empty() {
            self.line(depth, &format!("{callback} => {{ {} }}", body.join(" ")));
        }
    }

    /// `visible` from `hidden`, and the accessible name of kinds that do not show `label`.
    fn common(&mut self, depth: usize, node: &Node, labelled: bool) {
        if node.props.contains_key("hidden")
            && let Some(v) = self.hidden(node)
        {
            self.line(depth, &format!("visible: {v};"));
        }
        if labelled {
            self.bind_line(depth, node, "label", "accessible-label", Ty::Str);
        }
    }

    fn hidden(&mut self, node: &Node) -> Option<String> {
        match node.props.get("hidden")? {
            Value::Bool(b) => Some((!b).to_string()),
            Value::Bind { bind, not } => self.data.read(bind, !not, Ty::Bool),
            _ => None,
        }
    }

    fn open(&mut self, depth: usize, node: &Node, widget: &str, place: &[String]) {
        let head = match &node.id {
            Some(id) => format!("{id} := {widget} {{"),
            None => format!("{widget} {{"),
        };
        self.line(depth, &head);
        for p in place {
            self.line(depth + 1, p);
        }
    }

    fn screen(&mut self, root: &Node, name: &str) {
        self.line(0, &format!("export component {name} inherits Window {{"));
        self.bind_line(1, root, "label", "title", Ty::Str);
        let props: Vec<String> = self
            .data
            .props
            .values()
            .map(|p| format!("in-out property <{}> {};", p.ty.slint(), p.name))
            .collect();
        for p in props {
            self.line(1, &p);
        }
        self.line(1, "callback perform(string, string);");
        self.out.push('\n');
        self.imports.insert("VerticalBox");
        self.open(1, root, "VerticalBox", &[]);
        self.line(2, "alignment: LayoutAlignment.start;");
        self.children(root.children.iter(), 2);
        self.line(1, "}");
        self.line(0, "}");
    }

    fn children<'n>(&mut self, children: impl Iterator<Item = &'n Child>, depth: usize) {
        for child in children {
            if let Child::Node(n) = child {
                self.node(n, depth, &[]);
            }
        }
    }

    fn spacing(&mut self, node: &Node) -> Option<String> {
        let Value::Token(path) = node.props.get("gap")? else {
            self.unsupported(node, "a gap that is not a token");
            return None;
        };
        let token = self.tokens.get(path);
        let value = token.filter(|t| t.kind == "dimension").and_then(|t| {
            let n = t.value.get("value")?.as_f64()?;
            match t.value.get("unit")?.as_str()? {
                "px" => Some(n),
                "rem" => Some(n * PX_PER_REM),
                _ => None,
            }
        });
        if value.is_none() {
            self.unsupported(node, &format!("token {path} has no px value"));
        }
        value.map(|px| format!("spacing: {}px;", js_number(px)))
    }

    fn node(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = depth + 1;
        match node.kind.as_str() {
            "stack" => {
                let row =
                    matches!(node.props.get("direction"), Some(Value::String(s)) if s == "row");
                let layout = if row {
                    "HorizontalLayout"
                } else {
                    "VerticalLayout"
                };
                self.open(depth, node, layout, place);
                if let Some(s) = self.spacing(node) {
                    self.line(d, &s);
                }
                self.common(d, node, false);
                self.children(node.children.iter(), d);
            }
            "grid" => {
                let columns = match node.props.get("columns") {
                    Some(Value::Number(n)) if *n >= 1.0 => *n as usize,
                    _ => {
                        self.unsupported(node, "`columns` must be a literal for GridLayout");
                        1
                    }
                };
                self.open(depth, node, "GridLayout", place);
                if let Some(s) = self.spacing(node) {
                    self.line(d, &s);
                }
                self.common(d, node, false);
                let cells: Vec<&Node> = element_children(node).collect();
                for (i, cell) in cells.into_iter().enumerate() {
                    let at = [
                        format!("row: {};", i / columns),
                        format!("col: {};", i % columns),
                    ];
                    self.node(cell, d, &at);
                }
            }
            "section" => {
                self.imports.insert("GroupBox");
                self.open(depth, node, "GroupBox", place);
                self.bind_line(d, node, "label", "title", Ty::Str);
                self.common(d, node, false);
                self.line(d, "VerticalBox {");
                let header = node.slots.get("header").into_iter().flatten();
                self.children(header.chain(node.children.iter()), d + 1);
                self.line(d, "}");
            }
            "form" => {
                self.open(depth, node, "VerticalBox", place);
                self.common(d, node, false);
                self.forms.push(Self::call(node, "submit"));
                self.children(node.children.iter(), d);
                if let Some(footer) = node.slots.get("footer") {
                    self.imports.insert("HorizontalBox");
                    self.line(d, "HorizontalBox {");
                    self.children(footer.iter(), d + 1);
                    self.line(d, "}");
                }
                self.forms.pop();
            }
            "heading" | "text" => {
                self.open(depth, node, "Text", place);
                let text = self.text(node);
                self.line(d, &format!("text: {text};"));
                if node.kind == "heading" {
                    let level = match node.props.get("level") {
                        Some(Value::Number(n)) => (*n as usize).clamp(1, HEADING_FONT_PX.len()),
                        _ => {
                            self.unsupported(node, "a bound heading level");
                            1
                        }
                    };
                    self.line(d, &format!("font-size: {}px;", HEADING_FONT_PX[level - 1]));
                    self.line(d, &format!("font-weight: {HEADING_WEIGHT};"));
                }
                self.common(d, node, node.kind == "heading");
            }
            "link" => {
                self.imports.insert("Palette");
                self.open(depth, node, "TouchArea", place);
                self.common(d, node, false);
                self.handler(d, "clicked", &[Self::call(node, "press")]);
                let text = self.text(node);
                self.line(
                    d,
                    &format!("Text {{ text: {text}; color: Palette.accent-background; }}"),
                );
            }
            "button" => {
                self.imports.insert("Button");
                self.open(depth, node, "Button", place);
                let text = self.text(node);
                self.line(d, &format!("text: {text};"));
                match node.props.get("variant") {
                    Some(Value::String(v)) if v == "primary" => self.line(d, "primary: true;"),
                    Some(Value::Bind { .. }) => self.unsupported(node, "a bound variant"),
                    _ => {}
                }
                self.enabled(d, node);
                self.common(d, node, true);
                let submits = matches!(node.props.get("submit"), Some(Value::Bool(true)));
                let submit = if submits {
                    self.forms.last().cloned().flatten()
                } else {
                    None
                };
                self.handler(d, "clicked", &[Self::call(node, "press"), submit]);
            }
            "field" => self.field(node, depth, place),
            "checkbox" | "switch" => {
                let widget = if node.kind == "switch" {
                    "Switch"
                } else {
                    "CheckBox"
                };
                self.imports.insert(widget);
                self.open(depth, node, widget, place);
                self.bind_line(d, node, "label", "text", Ty::Str);
                self.bind_line(d, node, "checked", "checked", Ty::Bool);
                self.enabled(d, node);
                self.common(d, node, false);
                self.handler(d, "toggled", &[Self::call(node, "change")]);
            }
            "select" => self.select(node, depth, place),
            "slider" | "stepper" => {
                let (widget, ty) = if node.kind == "slider" {
                    ("Slider", Ty::Float)
                } else {
                    ("SpinBox", Ty::Int)
                };
                let d = self.captioned(node, depth, place);
                self.imports.insert(widget);
                self.open(d, node, widget, &[]);
                self.bind_line(d + 1, node, "min", "minimum", ty);
                self.bind_line(d + 1, node, "max", "maximum", ty);
                self.bind_line(d + 1, node, "value", "value", ty);
                self.enabled(d + 1, node);
                let callback = if node.kind == "slider" {
                    "changed"
                } else {
                    "edited"
                };
                self.handler(d + 1, callback, &[Self::call(node, "change")]);
                self.line(d, "}");
            }
            _ => {
                self.unsupported(node, "the Slint target does not map this kind yet (T67.2)");
                return;
            }
        }
        self.line(depth, "}");
    }

    /// `enabled` from `disabled`: the binding read negated, so no `!` piles up.
    fn enabled(&mut self, depth: usize, node: &Node) {
        let expr = match node.props.get("disabled") {
            None => return,
            Some(Value::Bool(b)) => Some((!b).to_string()),
            Some(Value::Bind { bind, not }) => self.data.read(bind, !not, Ty::Bool),
            Some(_) => None,
        };
        match expr {
            Some(e) => self.line(depth, &format!("enabled: {e};")),
            None => self.unsupported(node, "`disabled` has no Slint form"),
        }
    }

    /// A caption `Text` above a control that has none of its own, in a layout that takes the
    /// grid placement and `hidden`; returns the depth of the control. The caller closes it.
    fn captioned(&mut self, node: &Node, depth: usize, place: &[String]) -> usize {
        self.line(depth, "VerticalLayout {");
        for p in place {
            self.line(depth + 1, p);
        }
        self.common(depth + 1, node, false);
        if node.props.contains_key("label")
            && let Some(v) = self.value(node, "label", Ty::Str)
        {
            self.line(depth + 1, &format!("Text {{ text: {v}; }}"));
        }
        depth + 1
    }

    fn field(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = self.captioned(node, depth, place);
        let kind = match node.props.get("type") {
            Some(Value::String(s)) => s.as_str(),
            _ => "text",
        };
        let multiline = kind == "multiline";
        let widget = if multiline { "TextEdit" } else { "LineEdit" };
        self.imports.insert(widget);
        self.open(d, node, widget, &[]);
        match kind {
            "password" => self.line(d + 1, "input-type: InputType.password;"),
            "number" => self.line(d + 1, "input-type: InputType.number;"),
            _ => {}
        }
        self.bind_line(d + 1, node, "placeholder", "placeholder-text", Ty::Str);
        self.bind_line(d + 1, node, "value", "text", Ty::Str);
        self.enabled(d + 1, node);
        if !multiline {
            let submit = self.forms.last().cloned().flatten();
            self.handler(d + 1, "accepted", &[submit]);
        }
        self.handler(d + 1, "edited", &[Self::call(node, "change")]);
        self.line(d, "}");
    }

    fn select(&mut self, node: &Node, depth: usize, place: &[String]) {
        let mut labels = vec![];
        let mut values = vec![];
        for child in &node.children {
            let option = child.as_node().filter(|o| o.kind == "option");
            let literal = option.and_then(|o| match o.props.get("value") {
                Some(Value::String(v)) if !o.props.contains_key("text") => Some((o, v)),
                _ => None,
            });
            let Some((option, value)) = literal else {
                self.unsupported(
                    node,
                    "only options with literal values and text map to ComboBox",
                );
                return;
            };
            labels.push(self.text(option));
            values.push(string(value));
        }
        let d = self.captioned(node, depth, place);
        self.imports.insert("ComboBox");
        self.open(d, node, "ComboBox", &[]);
        self.line(d + 1, &format!("model: [{}];", labels.join(", ")));
        let mut write = None;
        match node.props.get("value") {
            Some(Value::Bind { bind, not: false }) => {
                let read = self.data.read(bind, false, Ty::Str).unwrap_or_default();
                let mut index = "0".to_owned();
                for (i, v) in values.iter().enumerate().skip(1).rev() {
                    index = format!("{read} == {v} ? {i} : {index}");
                }
                self.line(d + 1, &format!("current-index: {index};"));
                write = self.data.two_way(bind, false, Ty::Str).map(|target| {
                    format!("{target} = [{}][self.current-index];", values.join(", "))
                });
            }
            Some(Value::String(v)) => {
                let i = values.iter().position(|x| *x == string(v)).unwrap_or(0);
                self.line(d + 1, &format!("current-index: {i};"));
            }
            _ => {}
        }
        self.enabled(d + 1, node);
        self.handler(d + 1, "selected", &[write, Self::call(node, "change")]);
        self.line(d, "}");
    }
}
