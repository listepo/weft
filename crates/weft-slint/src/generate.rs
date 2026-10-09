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
use crate::names::{clash_key, element_name, identifier, string, type_name};

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
    // A use means its fragment's body (SPEC §10.7); validation reported any cycle or excess.
    let expanded = weft_core::expand(document, options.catalog).document;
    let document = &expanded;
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
        repeat: None,
        aliases: vec![],
        hooks: vec![],
        inits: vec![],
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
    if let Some(id) = node.id.as_deref().map(element_name) {
        let id = &id;
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
    /// `for <as> in <model>:` waiting for the repeated element's opening line.
    repeat: Option<String>,
    /// `<each as>` names in scope. An element id equal to one is left off: Slint would treat it
    /// as the repeater variable.
    aliases: Vec<String>,
    hooks: Vec<String>,
    inits: Vec<String>,
}

impl Gen<'_> {
    fn line(&mut self, depth: usize, text: &str) {
        self.out.push_str(&INDENT.repeat(depth));
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// The next element opened is the body of a `for`, so the prefix has to sit on its first line.
    fn emit(&mut self, depth: usize, text: &str) {
        match self.repeat.take() {
            Some(prefix) => self.line(depth, &format!("{prefix}{text}")),
            None => self.line(depth, text),
        }
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
        let head = match node.id.as_deref().map(element_name) {
            Some(id) if !self.aliases.contains(&id) => {
                format!("{id} := {widget} {{")
            }
            _ => format!("{widget} {{"),
        };
        self.emit(depth, &head);
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
        let models: Vec<String> = self
            .data
            .models
            .values()
            .map(|model| format!("in-out property <{}> {};", model.slint_type(), model.name))
            .collect();
        for model in models {
            self.line(1, &model);
        }
        self.line(1, "callback perform(string, string);");
        self.out.push('\n');
        // A MenuBar is legal only as a direct child of Window, so every menu is emitted here.
        self.hoist_menus(root);
        self.imports.insert("VerticalBox");
        self.open(1, root, "VerticalBox", &[]);
        self.line(2, "alignment: LayoutAlignment.start;");
        self.children(root.children.iter(), 2);
        self.line(1, "}");
        let inits = self.inits.clone();
        if !inits.is_empty() {
            self.line(1, "init => {");
            for init in &inits {
                self.line(2, init);
            }
            self.line(1, "}");
        }
        let hooks = self.hooks.clone();
        for hook in &hooks {
            self.line(1, hook);
        }
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
            "select" | "combobox" => {
                if !self.choice(node, depth, place) {
                    return;
                }
            }
            "each" => {
                self.each(node, depth, place);
                return;
            }
            "list" => self.list(node, depth, place),
            "item" => self.item(node, depth, place),
            "table" => self.table(node, depth, place),
            "tabs" => self.tabs(node, depth, place),
            "dialog" => self.dialog(node, depth, place),
            // Emitted beside the window's box: a MenuBar cannot sit in a layout.
            "menu" => return,
            "alert" => self.alert(node, depth, place),
            "image" | "model" => {
                // `@image-url` loads a file next to the `.slint`. A string `src` stays in the comment.
                self.open(depth, node, "Image", place);
                self.common(d, node, true);
            }
            "date-picker" | "color-picker" => self.string_input(node, depth, place),
            "radio-group" => self.exclusive(node, depth, place, "radio", false),
            "segmented-control" => self.exclusive(node, depth, place, "segment", true),
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
        self.emit(depth, "VerticalLayout {");
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

    /// `false` when nothing was opened, so the caller does not emit a closing brace.
    fn choice(&mut self, node: &Node, depth: usize, place: &[String]) -> bool {
        let kids: Vec<&Node> = element_children(node).collect();
        if kids.len() == 1 && kids[0].kind == "each" {
            return self.choice_model(node, kids[0], depth, place);
        }
        let mut labels = vec![];
        let mut values = vec![];
        for child in &node.children {
            let Some(option) = child.as_node().filter(|o| o.kind == "option") else {
                continue;
            };
            let Some(Value::String(value)) = option.props.get("value") else {
                self.unsupported(node, "only options with literal values map to ComboBox");
                return false;
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
        true
    }

    fn choice_model(&mut self, node: &Node, each: &Node, depth: usize, place: &[String]) -> bool {
        let Some(model) = self.model_expr(each) else {
            self.unsupported(each, "the options model has no Slint form");
            return false;
        };
        let d = self.captioned(node, depth, place);
        self.imports.insert("ComboBox");
        self.open(d, node, "ComboBox", &[]);
        self.line(d + 1, &format!("model: {model};"));
        self.bind_line(d + 1, node, "value", "current-value", Ty::Str);
        self.enabled(d + 1, node);
        self.handler(d + 1, "selected", &[Self::call(node, "change")]);
        self.line(d, "}");
        true
    }

    fn model_expr(&self, each: &Node) -> Option<String> {
        let Value::Bind { bind, not: false } = each.props.get("in")? else {
            return None;
        };
        self.data
            .models
            .get(bind)
            .map(|model| format!("root.{}", model.name))
    }

    fn loop_alias<'a>(&self, each: &'a Node) -> Option<&'a str> {
        match each.props.get("as")? {
            Value::String(name) => identifier(name),
            _ => None,
        }
    }

    fn each(&mut self, node: &Node, depth: usize, place: &[String]) {
        let Some(alias) = self.loop_alias(node).map(str::to_owned) else {
            self.unsupported(node, "the loop variable is reserved in Slint");
            return;
        };
        let Some(model) = self.model_expr(node) else {
            self.unsupported(node, "the list model has no Slint form");
            return;
        };
        let kids: Vec<&Node> = element_children(node).collect();
        self.aliases.push(alias.clone());
        match kids.as_slice() {
            [only] => {
                self.repeat = Some(format!("for {alias} in {model}: "));
                self.node(only, depth, place);
                self.repeat = None;
            }
            [] => {}
            many => {
                self.line(depth, &format!("for {alias} in {model}: VerticalLayout {{"));
                for child in many {
                    self.node(child, depth + 1, &[]);
                }
                self.line(depth, "}");
            }
        }
        self.aliases.pop();
    }

    fn list(&mut self, node: &Node, depth: usize, place: &[String]) {
        let elements: Vec<&Node> = element_children(node).collect();
        let (eachs, items): (Vec<&Node>, Vec<&Node>) = elements
            .iter()
            .copied()
            .partition(|child| child.kind == "each");
        let empty = node.slots.get("empty");
        // A ListView's only child element is one `for`. Anything else is a plain column.
        let bare = eachs.len() == 1 && items.is_empty() && empty.is_none();
        let d = depth + 1;
        if bare {
            self.imports.insert("ListView");
            self.open(depth, node, "ListView", place);
            self.common(d, node, false);
            self.each(eachs[0], d, &[]);
            return;
        }
        self.open(depth, node, "VerticalLayout", place);
        self.common(d, node, false);
        if let Some(each) = eachs.first() {
            self.imports.insert("ListView");
            self.line(d, "ListView {");
            self.each(each, d + 1, &[]);
            self.line(d, "}");
            if let (Some(slot), Some(model)) = (empty, self.model_expr(each)) {
                self.line(d, &format!("if {model}.length == 0: VerticalLayout {{"));
                self.children(slot.iter(), d + 1);
                self.line(d, "}");
            }
        } else if let Some(slot) = empty {
            self.children(slot.iter(), d);
        }
        for item in items {
            self.node(item, d, &[]);
        }
        for each in eachs.iter().skip(1) {
            self.each(each, d, &[]);
        }
    }

    fn item(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = depth + 1;
        let press = Self::call(node, "press");
        let elements: Vec<&Node> = element_children(node).collect();
        let has_text = node.props.contains_key("text")
            || node
                .children
                .iter()
                .any(|child| matches!(child, Child::Text(_)));
        if press.is_none() && elements.is_empty() {
            let text = self.text(node);
            self.open(depth, node, "Text", place);
            self.line(d, &format!("text: {text};"));
            self.common(d, node, false);
            return;
        }
        let widget = if press.is_some() {
            "TouchArea"
        } else {
            "VerticalLayout"
        };
        self.open(depth, node, widget, place);
        self.common(d, node, false);
        self.handler(d, "clicked", &[press]);
        let text = has_text.then(|| self.text(node));
        if elements.is_empty() {
            self.line(
                d,
                &format!("Text {{ text: {}; }}", text.unwrap_or_default()),
            );
            return;
        }
        if widget == "TouchArea" {
            self.line(d, "VerticalLayout {");
            if let Some(text) = text {
                self.line(d + 1, &format!("Text {{ text: {text}; }}"));
            }
            self.children(node.children.iter(), d + 1);
            self.line(d, "}");
        } else if let Some(text) = text {
            self.line(d, &format!("Text {{ text: {text}; }}"));
            self.children(node.children.iter(), d);
        } else {
            self.children(node.children.iter(), d);
        }
    }

    fn table(&mut self, node: &Node, depth: usize, place: &[String]) {
        let elements: Vec<&Node> = element_children(node).collect();
        let columns: Vec<&Node> = elements
            .iter()
            .copied()
            .filter(|n| n.kind == "column")
            .collect();
        let rows: Vec<&Node> = elements
            .iter()
            .copied()
            .filter(|n| n.kind == "row")
            .collect();
        let eachs: Vec<&Node> = elements
            .iter()
            .copied()
            .filter(|n| n.kind == "each")
            .collect();
        let empty = node.slots.get("empty");
        self.imports.insert("StandardTableView");
        let d = depth + 1;
        if empty.is_some() {
            self.open(depth, node, "VerticalLayout", place);
            self.common(d, node, false);
            self.line(d, "StandardTableView {");
            self.table_body(node, &columns, &rows, &eachs, d + 1, false);
            self.line(d, "}");
            self.table_empty(empty, eachs.first().copied(), d);
        } else {
            self.open(depth, node, "StandardTableView", place);
            self.table_body(node, &columns, &rows, &eachs, d, true);
        }
    }

    fn table_body(
        &mut self,
        node: &Node,
        columns: &[&Node],
        rows: &[&Node],
        eachs: &[&Node],
        depth: usize,
        hidden: bool,
    ) {
        self.bind_line(depth, node, "label", "accessible-label", Ty::Str);
        if hidden {
            self.common(depth, node, false);
        }
        let cols: Vec<String> = columns.iter().map(|col| self.column_value(col)).collect();
        self.line(depth, &format!("columns: [{}];", cols.join(", ")));
        if let Some(body) = self.static_rows(rows) {
            self.line(depth, &body);
        }
        if let Some(index) = rows
            .iter()
            .position(|row| matches!(row.props.get("selected"), Some(Value::Bool(true))))
        {
            self.line(depth, &format!("current-row: {index};"));
        }
        self.sort_handler(columns, depth, "sort-ascending");
        self.sort_handler(columns, depth, "sort-descending");
        if let Some(call) = row_press(rows, eachs) {
            self.line(
                depth,
                &format!(
                    "row-pointer-event(row, event, position) => {{ if event.kind == PointerEventKind.up {{ {call} }} }}"
                ),
            );
        }
    }

    fn table_empty(&mut self, empty: Option<&Vec<Child>>, each: Option<&Node>, depth: usize) {
        let Some(slot) = empty else { return };
        if let Some(model) = each.and_then(|each| self.model_expr(each)) {
            self.line(depth, &format!("if {model}.length == 0: VerticalLayout {{"));
            self.children(slot.iter(), depth + 1);
            self.line(depth, "}");
        } else {
            self.children(slot.iter(), depth);
        }
    }

    fn column_value(&mut self, column: &Node) -> String {
        let title = self.text(column);
        let sort = match column.props.get("sort") {
            Some(Value::String(order)) if order == "ascending" => "ascending",
            Some(Value::String(order)) if order == "descending" => "descending",
            _ => return format!("{{ title: {title} }}"),
        };
        format!("{{ title: {title}, sort-order: SortOrder.{sort} }}")
    }

    fn static_rows(&mut self, rows: &[&Node]) -> Option<String> {
        if rows.is_empty() {
            return None;
        }
        let mut rendered = Vec::new();
        for row in rows {
            let cells: Vec<String> = element_children(row)
                .filter(|cell| cell.kind == "cell")
                .map(|cell| format!("{{ text: {} }}", self.cell_text(cell)))
                .collect();
            rendered.push(format!("[{}]", cells.join(", ")));
        }
        Some(format!("rows: [{}];", rendered.join(", ")))
    }

    fn cell_text(&mut self, cell: &Node) -> String {
        if cell.props.contains_key("text")
            || cell
                .children
                .iter()
                .any(|child| matches!(child, Child::Text(_)))
        {
            return self.text(cell);
        }
        let kids: Vec<&Node> = element_children(cell).collect();
        if kids.len() == 1 && matches!(kids[0].kind.as_str(), "text" | "heading") {
            return self.text(kids[0]);
        }
        "\"\"".into()
    }

    fn sort_handler(&mut self, columns: &[&Node], depth: usize, callback: &str) {
        let arms: Vec<String> = columns
            .iter()
            .enumerate()
            .filter_map(|(index, column)| {
                Self::call(column, "press").map(|call| format!("if column == {index} {{ {call} }}"))
            })
            .collect();
        if !arms.is_empty() {
            self.line(
                depth,
                &format!("{callback}(column) => {{ {} }}", arms.join(" ")),
            );
        }
    }

    fn tabs(&mut self, node: &Node, depth: usize, place: &[String]) {
        self.imports.insert("TabWidget");
        self.open(depth, node, "TabWidget", place);
        let d = depth + 1;
        // A label on its own is refused: the role has to be set in the same element.
        if node.props.contains_key("label") {
            self.line(d, "accessible-role: AccessibleRole.tab-list;");
        }
        self.bind_line(d, node, "label", "accessible-label", Ty::Str);
        self.common(d, node, false);
        let tabs: Vec<&Node> = element_children(node)
            .filter(|child| child.kind == "tab")
            .collect();
        self.tab_selection(node, &tabs, d);
        for tab in tabs {
            self.tab(tab, d);
        }
    }

    fn tab_selection(&mut self, node: &Node, tabs: &[&Node], depth: usize) {
        let ids: Vec<String> = tabs
            .iter()
            .map(|tab| tab.id.clone().unwrap_or_default())
            .collect();
        let call = Self::call(node, "change");
        match node.props.get("selected") {
            Some(Value::Bind { bind, not: false }) => {
                let Some(read) = self.data.read(bind, false, Ty::Str) else {
                    return;
                };
                let mut index = "0".to_owned();
                for (at, id) in ids.iter().enumerate().skip(1).rev() {
                    index = format!("{read} == {} ? {at} : {index}", string(id));
                }
                self.line(depth, &format!("current-index: {index};"));
                let list = ids
                    .iter()
                    .map(|id| string(id))
                    .collect::<Vec<_>>()
                    .join(", ");
                let write = self
                    .data
                    .two_way(bind, false, Ty::Str)
                    .map(|target| format!("{target} = [{list}][self.current-index];"));
                self.handler(depth, "changed current-index", &[write, call]);
            }
            Some(Value::String(id)) => {
                let at = ids
                    .iter()
                    .position(|candidate| candidate == id)
                    .unwrap_or(0);
                self.line(depth, &format!("current-index: {at};"));
                self.handler(depth, "changed current-index", &[call]);
            }
            _ => self.handler(depth, "changed current-index", &[call]),
        }
    }

    fn tab(&mut self, node: &Node, depth: usize) {
        let d = depth + 1;
        match node.id.as_deref().map(element_name) {
            Some(id) => self.line(depth, &format!("{id} := Tab {{")),
            None => self.line(depth, "Tab {"),
        }
        self.bind_line(d, node, "label", "title", Ty::Str);
        self.line(d, "VerticalLayout {");
        self.children(node.children.iter(), d + 1);
        self.line(d, "}");
        self.line(depth, "}");
    }

    fn dialog(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = depth + 1;
        self.open(depth, node, "PopupWindow", place);
        // A modal dialog does not dismiss on an outside click; PopupWindow is not itself modal.
        if !matches!(node.props.get("modal"), Some(Value::Bool(false))) {
            self.line(d, "close-policy: PopupClosePolicy.no-auto-close;");
        }
        self.line(d, "VerticalLayout {");
        if node.props.contains_key("label")
            && let Some(title) = self.value(node, "label", Ty::Str)
        {
            self.line(d + 1, &format!("Text {{ text: {title}; }}"));
        }
        self.children(node.children.iter(), d + 1);
        if let Some(actions) = node.slots.get("actions") {
            self.imports.insert("HorizontalBox");
            self.line(d + 1, "HorizontalBox {");
            self.children(actions.iter(), d + 2);
            self.line(d + 1, "}");
        }
        self.line(d, "}");
        self.dialog_open(node);
    }

    fn dialog_open(&mut self, node: &Node) {
        let Some(id) = node.id.as_deref().map(element_name) else {
            return;
        };
        let Some(Value::Bind { bind, not }) = node.props.get("open") else {
            if matches!(node.props.get("open"), Some(Value::Bool(true))) {
                self.inits.push(format!("{id}.show();"));
            }
            return;
        };
        let Some(show) = self.data.read(bind, *not, Ty::Bool) else {
            self.unsupported(node, "`open` has no Slint form");
            return;
        };
        let Some((name, bool_prop)) = self
            .data
            .props
            .get(bind)
            .map(|prop| (prop.name.clone(), prop.ty == Ty::Bool))
        else {
            return;
        };
        self.hooks.push(format!(
            "changed {name} => {{ if {show} {{ {id}.show(); }} else {{ {id}.close(); }} }}"
        ));
        self.inits.push(format!("if {show} {{ {id}.show(); }}"));
        let mut stmts = Vec::new();
        if bool_prop {
            stmts.push(format!("root.{name} = {not};"));
        }
        if let Some(call) = Self::call(node, "close") {
            stmts.push(call);
        }
        if !stmts.is_empty() {
            // `is-open` is read from the parent: inside the popup the compiler
            // lowers the element to a window and that property is gone.
            let shown = format!("{id}-is-open");
            self.hooks
                .push(format!("property <bool> {shown}: {id}.is-open;"));
            self.hooks.push(format!(
                "changed {shown} => {{ if !{shown} && {show} {{ {} }} }}",
                stmts.join(" ")
            ));
        }
    }

    fn hoist_menus(&mut self, node: &Node) {
        if node.kind == "menu" {
            self.menu(node, 1, &[]);
        }
        for child in &node.children {
            if let Child::Node(inner) = child {
                self.hoist_menus(inner);
            }
        }
        for slot in node.slots.values() {
            for child in slot {
                if let Child::Node(inner) = child {
                    self.hoist_menus(inner);
                }
            }
        }
    }

    fn menu(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = depth + 1;
        self.open(depth, node, "MenuBar", place);
        self.line(d, "Menu {");
        self.bind_line(d + 1, node, "label", "title", Ty::Str);
        for child in element_children(node) {
            if child.kind == "menu-item" {
                self.menu_item(child, d + 1);
            } else {
                self.unsupported(child, "a menu holds menu items");
            }
        }
        self.line(d, "}");
        self.line(depth, "}");
    }

    fn menu_item(&mut self, node: &Node, depth: usize) {
        let d = depth + 1;
        match node.id.as_deref().map(element_name) {
            Some(id) => self.line(depth, &format!("{id} := MenuItem {{")),
            None => self.line(depth, "MenuItem {"),
        }
        let title = self.text(node);
        self.line(d, &format!("title: {title};"));
        self.enabled(d, node);
        self.handler(d, "activated", &[Self::call(node, "press")]);
        self.line(depth, "}");
    }

    fn alert(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = depth + 1;
        self.open(depth, node, "VerticalLayout", place);
        self.common(d, node, false);
        let has_text = node.props.contains_key("text")
            || node
                .children
                .iter()
                .any(|child| matches!(child, Child::Text(_)));
        if has_text {
            let text = self.text(node);
            self.line(d, &format!("Text {{ text: {text}; }}"));
        }
        self.children(node.children.iter(), d);
    }

    /// `DatePickerPopup` holds a `Date` and `TimePickerPopup` a `Time`. The prop is a string, so
    /// the value stays editable; `type`, `min` and `max` have no property on `LineEdit`.
    fn string_input(&mut self, node: &Node, depth: usize, place: &[String]) {
        let d = self.captioned(node, depth, place);
        self.imports.insert("LineEdit");
        self.open(d, node, "LineEdit", &[]);
        self.bind_line(d + 1, node, "value", "text", Ty::Str);
        self.enabled(d + 1, node);
        self.handler(d + 1, "edited", &[Self::call(node, "change")]);
        self.line(d, "}");
    }

    fn exclusive(
        &mut self,
        node: &Node,
        depth: usize,
        place: &[String],
        child_kind: &str,
        horizontal: bool,
    ) {
        let d = depth + 1;
        self.imports.insert("RadioGroup");
        self.open(depth, node, "RadioGroup", place);
        self.bind_line(d, node, "label", "title", Ty::Str);
        if horizontal {
            self.line(d, "orientation: Orientation.horizontal;");
        }
        self.enabled(d, node);
        self.common(d, node, false);
        let mut pairs = Vec::new();
        for child in element_children(node) {
            if child.kind != child_kind {
                self.unsupported(child, "this group only holds its options");
                continue;
            }
            let Some(Value::String(value)) = child.props.get("value") else {
                self.unsupported(child, "the option value must be a literal");
                continue;
            };
            let label = self.text(child);
            pairs.push((label, string(value), child));
        }
        let selected = match node.props.get("value") {
            Some(Value::Bind { bind, not: false }) => self.data.read(bind, false, Ty::Str),
            Some(Value::String(value)) => Some(string(value)),
            _ => None,
        };
        for (label, value, child) in &pairs {
            let inner = d + 1;
            match child.id.as_deref().map(element_name) {
                Some(id) => self.line(d, &format!("{id} := RadioButton {{")),
                None => self.line(d, "RadioButton {"),
            }
            self.line(inner, &format!("text: {label};"));
            self.enabled(inner, child);
            if let Some(current) = &selected {
                self.line(inner, &format!("checked: {current} == {value};"));
            }
            self.line(d, "}");
        }
        let write = match node.props.get("value") {
            Some(Value::Bind { bind, not: false }) => self.data.two_way(bind, false, Ty::Str),
            _ => None,
        };
        let assign = write.map(|target| {
            let mut expr = pairs
                .last()
                .map(|(_, value, _)| value.clone())
                .unwrap_or_else(|| "\"\"".into());
            for (label, value, _) in pairs.iter().rev().skip(1) {
                expr = format!("picked == {label} ? {value} : {expr}");
            }
            format!("{target} = {expr};")
        });
        if assign.is_some() {
            let body = [assign, Self::call(node, "change")];
            let text: Vec<&str> = body.iter().flatten().map(String::as_str).collect();
            if !text.is_empty() {
                self.line(d, &format!("selected(picked) => {{ {} }}", text.join(" ")));
            }
        } else {
            self.handler(d, "selected", &[Self::call(node, "change")]);
        }
    }
}

fn row_press(rows: &[&Node], eachs: &[&Node]) -> Option<String> {
    let found = rows.iter().copied();
    let nested = eachs.iter().flat_map(|each| element_children(each));
    found.chain(nested).find_map(|row| {
        if row.kind == "row" {
            Gen::call(row, "press")
        } else {
            None
        }
    })
}
