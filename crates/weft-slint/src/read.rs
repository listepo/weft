//! Slint that `import_slint` rejects, from the syntax tree (SPEC §9, "From Slint").
//! Parsed, never compiled or run. A brace that would read as a binding is escaped
//! the way the other importers escape it.

use std::collections::HashMap;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::literals::unescape_string;
use i_slint_compiler::parser::{self, NodeOrToken, SyntaxKind, SyntaxNode};
use indexmap::IndexMap;
use weft_catalog::{Token, token_types};
use weft_core::{
    Catalog, Child, Code, Diagnostic, Document, Mode, Node, ParseOptions, Value, WEFT_VERSION,
    has_errors, is_id, parse,
};
use weft_import::{
    ImportResult, LossKind, Losses, MAX_DEPTH, MAX_NODES, empty_result, limit_reached, literal,
};

use crate::data::Ty;
use crate::generate::MARKER;
use crate::import::{ImportOptions, MAX_SOURCE_LENGTH};
use crate::read_expr::{self, Index};

/// Reads `source` from the Slint syntax tree. An exact generated file belongs to `import_slint`.
pub fn read_slint(source: &str, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics = vec![];
    if source.len() > MAX_SOURCE_LENGTH {
        let what = format!("is longer than {MAX_SOURCE_LENGTH} bytes");
        limit_reached(&mut diagnostics, "#", &what);
        return empty_result(diagnostics);
    }
    let mut built = BuildDiagnostics::default();
    let tree = parser::parse(source.to_owned(), None, &mut built);
    let Some(component) = component_of(&tree) else {
        return missing(diagnostics);
    };
    let Some(element) = component.child_node(SyntaxKind::Element) else {
        return missing(diagnostics);
    };
    let mut reader = Reader::new(
        options.catalog,
        options.tokens,
        read_expr::properties(&element),
    );
    if built.has_errors() {
        reader.lose(
            LossKind::Structure,
            "/",
            "syntax errors; parts were skipped",
        );
    }
    let mut root = reader.screen(&element);
    reader.finish(&mut root);
    if reader.limited {
        limit_reached(&mut diagnostics, "#", "is past the import limit");
    }
    if let Some(markup) = comment_markup(source) {
        let path = path_of("", &root);
        reader.lose(
            LossKind::Props,
            &path,
            "comment-only properties are not in the tree",
        );
        note_edits(&root, &markup, options, &mut reader.losses);
    }
    ImportResult {
        document: Document {
            weft: WEFT_VERSION.into(),
            root,
        },
        losses: reader.losses.0,
        diagnostics,
    }
}

fn missing(mut diagnostics: Vec<Diagnostic>) -> ImportResult {
    let diagnostic = Diagnostic::new(
        Code::W601,
        "#",
        "The source declares no component.",
        "a Slint component",
    );
    diagnostics.push(diagnostic);
    empty_result(diagnostics)
}

struct Reader<'a> {
    catalog: &'a Catalog,
    tokens: &'a IndexMap<String, Token>,
    props: read_expr::Props,
    losses: Losses,
    ids: weft_import::IdState,
    pending: Vec<Pending>,
    nodes: usize,
    limited: bool,
}

struct Pending {
    id: String,
    event: &'static str,
    action: String,
}

/// A label or `hidden` that belongs to the control inside a caption layout, not to the layout.
struct Around {
    label: Option<Fact>,
    hidden: Option<Value>,
}

#[derive(Clone)]
struct Fact {
    name: String,
    raw: String,
    string: Option<String>,
    expr: Option<SyntaxNode>,
}

impl<'a> Reader<'a> {
    fn new(
        catalog: &'a Catalog,
        tokens: &'a IndexMap<String, Token>,
        props: read_expr::Props,
    ) -> Self {
        Reader {
            catalog,
            tokens,
            props,
            losses: Losses::default(),
            ids: weft_import::IdState::default(),
            pending: Vec::new(),
            nodes: 0,
            limited: false,
        }
    }

    fn lose(&mut self, kind: LossKind, path: &str, note: &str) {
        self.losses.push(kind, path, note);
    }

    fn finish(&mut self, root: &mut Node) {
        let pending = std::mem::take(&mut self.pending);
        for item in pending {
            match place(root, &item) {
                Hit::Ok => {}
                Hit::Clash => self.lose(
                    LossKind::Actions,
                    &format!("#{}", item.id),
                    "a callback names two actions",
                ),
                Hit::Miss => {
                    let note =
                        format!("`{}` names an element that is not in the tree", item.action);
                    self.lose(LossKind::Actions, &format!("#{}", item.id), &note);
                }
            }
        }
    }

    fn screen(&mut self, window: &SyntaxNode) -> Node {
        self.nodes += 1;
        let mut root = Node::new("screen");
        let items = subs(window);
        let (explicit, body) = if base_name(window) == "Window" || base_name(window).is_empty() {
            plain_box(&items).unwrap_or((None, items))
        } else {
            (None, vec![window.clone()])
        };
        root.id = Some(self.fresh(explicit.as_deref(), "screen", "", "/screen"));
        let here = path_of("", &root);
        self.attr(&mut root, "label", &facts(window), "title", &here);
        root.children = self.kids(&body, &here, 1);
        root
    }

    fn kids(&mut self, items: &[SyntaxNode], parent: &str, depth: usize) -> Vec<Child> {
        let mut out = vec![];
        for sub in items {
            if sub.kind() == SyntaxKind::RepeatedElement {
                self.lose(LossKind::Repetition, parent, "a for is read once");
            }
            if sub.kind() == SyntaxKind::ConditionalElement {
                self.lose(LossKind::Hidden, parent, "an if is read as shown");
            }
            let host = inner_host(sub);
            let Some(host) = host else { continue };
            let el = element_of(&host);
            let Some(el) = el else { continue };
            let id = sub_id(&host);
            if id.is_none()
                && base_name(&el) == "VerticalLayout"
                && let Some(folded) = self.fold_caption(&el, parent, depth)
            {
                out.extend(folded);
                continue;
            }
            out.extend(self.one(
                &el,
                id,
                parent,
                depth,
                Around {
                    label: None,
                    hidden: None,
                },
            ));
        }
        out
    }

    fn one(
        &mut self,
        el: &SyntaxNode,
        id: Option<String>,
        parent: &str,
        depth: usize,
        around: Around,
    ) -> Vec<Child> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            self.limited = true;
            return vec![];
        }
        let base = base_name(el);
        let Some(kind) = kind_of(&base) else {
            let note = format!("`{base}` has no Weft kind; its children are kept");
            self.lose(LossKind::Kinds, parent, &note);
            return self.kids(&subs(el), parent, depth);
        };
        self.nodes += 1;
        let hint = string_of(&facts(el), "text");
        let mut node = Node::new(kind);
        let stem = format!("{parent}/{kind}");
        node.id = Some(self.fresh(id.as_deref(), kind, &hint, &stem));
        let here = path_of(parent, &node);
        let extra = self.fill(&mut node, &base, el, &here, depth);
        if let Some(fact) = around.label
            && !node.props.contains_key("label")
        {
            self.attr(&mut node, "label", &[fact], "text", &here);
        }
        if let Some(hidden) = around.hidden
            && !node.props.contains_key("hidden")
        {
            node.props.insert("hidden".into(), hidden);
        }
        self.blank_label(&mut node, &here);
        let mut out = vec![Child::Node(Box::new(node))];
        out.extend(extra);
        out
    }

    fn fill(
        &mut self,
        node: &mut Node,
        base: &str,
        el: &SyntaxNode,
        path: &str,
        depth: usize,
    ) -> Vec<Child> {
        let fs = facts(el);
        self.box_losses(node, base, path);
        self.note_rest(node, base, &fs, path);
        let early = match base {
            "Text" | "Button" => {
                if base == "Text" {
                    self.heading(node, &fs, path);
                }
                self.words(node, &fs, "text", path);
                None
            }
            "LineEdit" | "TextEdit" => {
                self.attr_ty(node, "value", &fs, "text", Ty::Str, path);
                self.attr_ty(node, "placeholder", &fs, "placeholder-text", Ty::Str, path);
                None
            }
            "CheckBox" | "Switch" => {
                self.attr_ty(node, "label", &fs, "text", Ty::Str, path);
                self.attr_ty(node, "checked", &fs, "checked", Ty::Bool, path);
                None
            }
            "ComboBox" => {
                self.combobox(node, el, &fs, path);
                None
            }
            "Slider" | "SpinBox" => {
                let ty = if base == "SpinBox" {
                    Ty::Int
                } else {
                    Ty::Float
                };
                self.attr_ty(node, "min", &fs, "minimum", ty, path);
                self.attr_ty(node, "max", &fs, "maximum", ty, path);
                self.attr_ty(node, "value", &fs, "value", ty, path);
                None
            }
            "TouchArea" => Some(self.link(node, el, path, depth)),
            "GroupBox" => {
                self.attr_ty(node, "label", &fs, "title", Ty::Str, path);
                None
            }
            _ => None,
        };
        self.flip(node, &fs, "enabled", "disabled", path);
        self.flip(node, &fs, "visible", "hidden", path);
        if !node.props.contains_key("label") {
            self.attr_ty(node, "label", &fs, "accessible-label", Ty::Str, path);
        }
        self.actions(node, el, path);
        if let Some(extra) = early {
            return extra;
        }
        if base == "TextEdit" {
            let multiline = Value::String("multiline".into());
            node.props.insert("type".into(), multiline);
        }
        // `ComboBox`, `CheckBox` and `SpinBox` end in `Box` and are not layouts.
        let nest = matches!(
            base,
            "VerticalLayout"
                | "HorizontalLayout"
                | "GridLayout"
                | "VerticalBox"
                | "HorizontalBox"
                | "GroupBox"
        );
        if nest {
            let children = self.kids(&subs(el), path, depth + 1);
            if base == "GridLayout" {
                let count = children.len().max(1) as f64;
                node.props.insert("columns".into(), Value::Number(count));
            }
            node.children = children;
        }
        vec![]
    }

    fn note_rest(&mut self, node: &mut Node, base: &str, fs: &[Fact], path: &str) {
        if let Some(spacing) = named(fs, "spacing") {
            match spacing
                .expr
                .as_ref()
                .and_then(|expr| read_expr::spacing_token(expr, self.tokens))
            {
                Some(token) => {
                    node.props.insert("gap".into(), Value::Token(token));
                }
                None => self.lose(LossKind::Tokens, path, "spacing is not a dimension token"),
            }
        }
        if base != "Text" && named(fs, "font-size").is_some() {
            self.lose(LossKind::Layout, path, "font-size is not a heading level");
        }
        if fs
            .iter()
            .any(|fact| fact.name == "row" || fact.name == "col")
        {
            self.lose(
                LossKind::Layout,
                path,
                "a grid's row and col are not placed",
            );
        }
    }

    fn box_losses(&mut self, node: &mut Node, base: &str, path: &str) {
        if base.starts_with("Horizontal") {
            node.props
                .insert("direction".into(), Value::String("row".into()));
        }
        if base == "VerticalBox" {
            self.lose(LossKind::Kinds, path, "VerticalBox is read as a stack");
        }
        if base == "HorizontalBox" {
            self.lose(LossKind::Slots, path, "HorizontalBox is not a footer slot");
        }
        if base == "GridLayout" {
            self.lose(LossKind::Layout, path, "grid cells are not placed");
        }
        if base == "GroupBox" {
            self.lose(LossKind::Slots, path, "GroupBox has no header slot");
        }
    }

    fn link(&mut self, node: &mut Node, el: &SyntaxNode, path: &str, depth: usize) -> Vec<Child> {
        let mut extra = vec![];
        let mut took = false;
        for sub in subs(el) {
            let text = sub.child_node(SyntaxKind::Element);
            let text = text.filter(|e| !took && sub_id(&sub).is_none() && base_name(e) == "Text");
            if let Some(text) = text {
                self.words(node, &facts(&text), "text", path);
                let color = fact(&text, "color");
                if color.is_some_and(|c| c.raw != "Palette.accent-background") {
                    self.lose(LossKind::Layout, path, "link colour is not the accent");
                }
                took = true;
            } else {
                extra.push(sub);
            }
        }
        if !extra.is_empty() {
            self.lose(
                LossKind::Structure,
                path,
                "link children are kept beside it",
            );
        }
        self.kids(&extra, path, depth)
    }

    fn words(&mut self, node: &mut Node, facts: &[Fact], name: &str, path: &str) {
        let Some(fact) = named(facts, name) else {
            return;
        };
        if let Some(text) = &fact.string {
            let shown = literal(&mut self.losses, path, text);
            node.children.push(Child::Text(shown));
            return;
        }
        match fact
            .expr
            .as_ref()
            .and_then(|expr| read_expr::read_value(expr, Ty::Str, &self.props))
        {
            Some(Value::String(text)) => {
                let shown = literal(&mut self.losses, path, &text);
                node.children.push(Child::Text(shown));
            }
            Some(value) => {
                node.props.insert("text".into(), value);
            }
            None => self.lose(LossKind::Bindings, path, "the text expression is not kept"),
        }
    }

    fn attr(&mut self, node: &mut Node, prop: &str, facts: &[Fact], name: &str, path: &str) {
        self.attr_ty(node, prop, facts, name, Ty::Str, path);
    }

    fn attr_ty(
        &mut self,
        node: &mut Node,
        prop: &str,
        facts: &[Fact],
        name: &str,
        want: Ty,
        path: &str,
    ) {
        let Some(fact) = named(facts, name) else {
            return;
        };
        let value = if fact.string.is_some() && want == Ty::Str {
            fact.string.clone().map(Value::String)
        } else {
            fact.expr
                .as_ref()
                .and_then(|expr| read_expr::read_value(expr, want, &self.props))
        };
        match value {
            Some(Value::String(text)) => {
                let shown = literal(&mut self.losses, path, &text);
                node.props.insert(prop.into(), Value::String(shown));
            }
            Some(other) => {
                node.props.insert(prop.into(), other);
            }
            None => {
                let note = format!("`{name}` is not a literal");
                self.lose(LossKind::Bindings, path, &note);
            }
        }
    }

    fn heading(&mut self, node: &mut Node, fs: &[Fact], path: &str) {
        let Some(size) = named(fs, "font-size") else {
            return;
        };
        let level = named(fs, "font-weight").and_then(|weight| {
            size.expr
                .as_ref()
                .zip(weight.expr.as_ref())
                .and_then(|(size, weight)| read_expr::heading_level(size, weight))
        });
        match level {
            Some(level) => {
                node.kind = "heading".into();
                node.props
                    .insert("level".into(), Value::Number(f64::from(level)));
            }
            None => self.lose(LossKind::Layout, path, "font-size is not a heading level"),
        }
    }

    fn flip(&mut self, node: &mut Node, fs: &[Fact], from: &str, to: &str, path: &str) {
        let Some(fact) = named(fs, from) else {
            return;
        };
        if node.props.contains_key(to) {
            return;
        }
        match fact
            .expr
            .as_ref()
            .and_then(|expr| read_expr::flipped_bool(expr, &self.props))
        {
            Some(value) => {
                node.props.insert(to.into(), value);
            }
            None => {
                let note = format!("`{from}` is not a literal");
                self.lose(LossKind::Bindings, path, &note);
            }
        }
    }

    fn combobox(&mut self, node: &mut Node, el: &SyntaxNode, fs: &[Fact], path: &str) {
        let Some(model) = named(fs, "model") else {
            self.lose(LossKind::Values, path, "a ComboBox model is not read");
            return;
        };
        let Some(labels) = model.expr.as_ref().and_then(read_expr::string_list) else {
            self.lose(
                LossKind::Values,
                path,
                "a ComboBox model is not a list of string literals",
            );
            return;
        };
        let written = read_expr::callbacks(el, &self.props)
            .into_iter()
            .find_map(|callback| callback.values);
        let values = match &written {
            Some((_, values)) if values.len() == labels.len() => values.clone(),
            _ => labels.clone(),
        };
        if let Some(index) = named(fs, "current-index") {
            match index
                .expr
                .as_ref()
                .and_then(|expr| read_expr::current_index(expr, &self.props))
            {
                Some(Index::At(at)) => {
                    if let Some(value) = values.get(at) {
                        let shown = literal(&mut self.losses, path, value.as_str());
                        node.props.insert("value".into(), Value::String(shown));
                    } else {
                        self.lose(LossKind::Values, path, "current-index is outside the model");
                    }
                }
                Some(Index::Path(bind)) => {
                    node.props
                        .insert("value".into(), Value::Bind { bind, not: false });
                }
                None => self.lose(LossKind::Bindings, path, "`current-index` is not a literal"),
            }
        }
        for (label, value) in labels.iter().zip(values) {
            let mut option = Node::new("option");
            let id = self.fresh(None, "option", label, &format!("{path}/option"));
            option.id = Some(id);
            let shown = literal(&mut self.losses, path, value.as_str());
            option.props.insert("value".into(), Value::String(shown));
            let text = literal(&mut self.losses, path, label);
            option.children.push(Child::Text(text));
            node.children.push(Child::Node(Box::new(option)));
        }
    }

    fn actions(&mut self, node: &mut Node, el: &SyntaxNode, path: &str) {
        for callback in read_expr::callbacks(el, &self.props) {
            let event = read_expr::event_of(&callback.name);
            let mut kept = callback.values.is_some() && event.is_some();
            let mut applied = 0;
            for call in &callback.performs {
                let own = node.id.as_deref() == Some(call.id.as_str());
                if own && let Some(event) = event.filter(|name| *name != "submit") {
                    self.bind_action(node, event, &call.action, path);
                    kept = true;
                    applied += 1;
                } else if !own && matches!(callback.name.as_str(), "clicked" | "accepted") {
                    // The generator only names another element from a form's submit.
                    self.pending.push(Pending {
                        id: call.id.clone(),
                        event: "submit",
                        action: call.action.clone(),
                    });
                    if callback.name == "clicked" {
                        node.props
                            .entry("submit".into())
                            .or_insert(Value::Bool(true));
                    }
                    kept = true;
                    applied += 1;
                }
            }
            if !kept || callback.residue || applied != callback.performs.len() {
                self.lose(LossKind::Actions, path, "a callback is not kept");
            }
        }
    }

    fn bind_action(&mut self, node: &mut Node, event: &str, action: &str, path: &str) {
        if let Some(existing) = node.on.get(event) {
            if existing != action {
                self.lose(LossKind::Actions, path, "a callback names two actions");
            }
            return;
        }
        node.on.insert(event.into(), action.to_owned());
    }

    /// A caption `Text` above one control, in the layout `generate` wraps them in. The layout
    /// itself is not an element: its `row` and `col` still cannot be placed.
    fn fold_caption(&mut self, el: &SyntaxNode, parent: &str, depth: usize) -> Option<Vec<Child>> {
        if el
            .children()
            .any(|child| child.kind() == SyntaxKind::CallbackConnection)
        {
            return None;
        }
        let fs = facts(el);
        let plain = fs
            .iter()
            .all(|fact| matches!(fact.name.as_str(), "row" | "col" | "visible"));
        if !plain {
            return None;
        }
        let mut caption = None;
        let mut control = None;
        for sub in subs(el) {
            let host = inner_host(&sub)?;
            let child = element_of(&host)?;
            let base = base_name(&child);
            let caption_text =
                base == "Text" && sub_id(&sub).is_none() && caption.is_none() && control.is_none();
            if caption_text {
                let text = facts(&child);
                if text.len() == 1 && text[0].name == "text" && readable_text(&text[0], &self.props)
                {
                    caption = Some(text[0].clone());
                    continue;
                }
            }
            let widget = matches!(
                base.as_str(),
                "LineEdit" | "TextEdit" | "Slider" | "SpinBox" | "ComboBox"
            );
            if widget && control.is_none() {
                control = Some((child, sub_id(&host)));
                continue;
            }
            return None;
        }
        let (child, id) = control?;
        let hidden = named(&fs, "visible").and_then(|fact| {
            fact.expr
                .as_ref()
                .and_then(|expr| read_expr::flipped_bool(expr, &self.props))
        });
        if named(&fs, "visible").is_some() && hidden.is_none() {
            return None;
        }
        let kids = self.one(
            &child,
            id,
            parent,
            depth,
            Around {
                label: caption,
                hidden,
            },
        );
        if fs
            .iter()
            .any(|fact| fact.name == "row" || fact.name == "col")
            && let Some(Child::Node(node)) = kids.first()
        {
            let here = path_of(parent, node);
            self.lose(
                LossKind::Layout,
                &here,
                "a grid's row and col are not placed",
            );
        }
        Some(kids)
    }

    fn blank_label(&mut self, node: &mut Node, path: &str) {
        let def = self.catalog.components.get(&node.kind);
        let needs = def.and_then(|d| d.requires_label) == Some(true);
        if needs && !node.props.contains_key("label") {
            self.lose(LossKind::Names, path, "a required label is empty");
            node.props
                .insert("label".into(), Value::String(String::new()));
        }
    }

    fn fresh(&mut self, explicit: Option<&str>, kind: &str, name: &str, stem: &str) -> String {
        if let Some(id) = explicit
            && is_id(id)
            && !self.ids.used.contains(id)
        {
            self.ids.used.insert(id.to_owned());
            return id.to_owned();
        }
        let note = match explicit {
            Some(id) => format!("the id {id:?} is not a valid Weft id"),
            None => "an id was generated".to_owned(),
        };
        let id = self.ids.fresh(kind, name);
        let path = format!("{stem}#{id}");
        self.lose(LossKind::Ids, &path, &note);
        id
    }
}

fn kind_of(base: &str) -> Option<&'static str> {
    Some(match base {
        "VerticalLayout" | "VerticalBox" => "stack",
        "HorizontalLayout" | "HorizontalBox" => "stack",
        "GridLayout" => "grid",
        "GroupBox" => "section",
        "Text" => "text",
        "TouchArea" => "link",
        "Button" => "button",
        "LineEdit" | "TextEdit" => "field",
        "CheckBox" => "checkbox",
        "Switch" => "switch",
        "ComboBox" => "select",
        "Slider" => "slider",
        "SpinBox" => "stepper",
        _ => return None,
    })
}

fn readable_text(fact: &Fact, props: &read_expr::Props) -> bool {
    fact.string.is_some()
        || fact
            .expr
            .as_ref()
            .is_some_and(|expr| read_expr::read_value(expr, Ty::Str, props).is_some())
}

enum Hit {
    Ok,
    Clash,
    Miss,
}

fn place(node: &mut Node, pending: &Pending) -> Hit {
    if node.id.as_deref() == Some(pending.id.as_str()) {
        return match node.on.get(pending.event) {
            Some(existing) if existing == &pending.action => Hit::Ok,
            Some(_) => Hit::Clash,
            None => {
                node.on.insert(pending.event.into(), pending.action.clone());
                Hit::Ok
            }
        };
    }
    for child in &mut node.children {
        if let Child::Node(child) = child {
            match place(child, pending) {
                Hit::Miss => {}
                other => return other,
            }
        }
    }
    for list in node.slots.values_mut() {
        for child in list {
            if let Child::Node(child) = child {
                match place(child, pending) {
                    Hit::Miss => {}
                    other => return other,
                }
            }
        }
    }
    Hit::Miss
}

fn path_of(parent: &str, node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{parent}/{}#{id}", node.kind),
        None => format!("{parent}/{}", node.kind),
    }
}

fn shown(node: &Node) -> Option<String> {
    let mut parts = vec![];
    for child in &node.children {
        if let Child::Text(text) = child {
            parts.push(text.as_str());
        }
    }
    if !parts.is_empty() {
        return Some(parts.join(" "));
    }
    match node.props.get("label").or(node.props.get("text")) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn plain_box(items: &[SyntaxNode]) -> Option<(Option<String>, Vec<SyntaxNode>)> {
    if items.len() != 1 {
        return None;
    }
    let sub = items.first()?;
    if sub.kind() != SyntaxKind::SubElement {
        return None;
    }
    let el = sub.child_node(SyntaxKind::Element)?;
    let name = base_name(&el);
    if name != "VerticalBox" && name != "VerticalLayout" {
        return None;
    }
    let fs = facts(&el);
    let plain = fs
        .iter()
        .all(|f| f.name == "alignment" && f.raw == "LayoutAlignment.start");
    if !plain {
        return None;
    }
    let calls = el
        .children()
        .any(|n| n.kind() == SyntaxKind::CallbackConnection);
    if calls {
        return None;
    }
    Some((sub_id(sub), subs(&el)))
}

fn string_of(facts: &[Fact], name: &str) -> String {
    named(facts, name)
        .and_then(|f| f.string.clone())
        .unwrap_or_default()
}

fn named<'a>(facts: &'a [Fact], name: &str) -> Option<&'a Fact> {
    facts.iter().find(|f| f.name == name)
}

fn fact(el: &SyntaxNode, name: &str) -> Option<Fact> {
    facts(el).into_iter().find(|f| f.name == name)
}

fn facts(el: &SyntaxNode) -> Vec<Fact> {
    let mut out = vec![];
    for child in el.children() {
        let kind = match child.kind() {
            SyntaxKind::Binding => SyntaxKind::BindingExpression,
            SyntaxKind::TwoWayBinding => SyntaxKind::Expression,
            _ => continue,
        };
        let Some(name) = first_ident(&child) else {
            continue;
        };
        let expr = child.child_node(kind);
        let raw = expr.as_ref().map(expr_text).unwrap_or_default();
        let string = expr
            .as_ref()
            .and_then(sole_string)
            .filter(|_| raw.starts_with('"'));
        out.push(Fact {
            name,
            raw,
            string,
            expr,
        });
    }
    out
}

fn expr_text(node: &SyntaxNode) -> String {
    let text = node.text().to_string();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.trim_end_matches(';').trim().to_owned()
}

fn sole_string(expr: &SyntaxNode) -> Option<String> {
    let mut found: Option<Option<String>> = None;
    walk_strings(expr, &mut found);
    found?
}

fn walk_strings(node: &SyntaxNode, found: &mut Option<Option<String>>) {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::StringLiteral => {
                let value = unescape_string(token.text()).map(|s| s.to_string());
                *found = Some(if found.is_some() { None } else { value });
            }
            NodeOrToken::Node(inner) => walk_strings(&inner, found),
            NodeOrToken::Token(_) => {}
        }
    }
}

fn base_name(el: &SyntaxNode) -> String {
    let Some(name) = el.child_node(SyntaxKind::QualifiedName) else {
        return String::new();
    };
    let mut last = String::new();
    for child in name.children_with_tokens() {
        let Some(token) = child.into_token() else {
            continue;
        };
        if token.kind() == SyntaxKind::Identifier {
            last = token.text().to_owned();
        }
    }
    last
}

fn first_ident(node: &SyntaxNode) -> Option<String> {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Identifier => {
                return Some(token.text().to_owned());
            }
            NodeOrToken::Node(inner) if inner.kind() == SyntaxKind::DeclaredIdentifier => {
                return first_ident(&inner);
            }
            _ => {}
        }
    }
    None
}

fn sub_id(sub: &SyntaxNode) -> Option<String> {
    if sub.kind() != SyntaxKind::SubElement {
        return None;
    }
    let mut tokens = vec![];
    for child in sub.children_with_tokens() {
        let Some(token) = child.into_token() else {
            continue;
        };
        let skip = matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comment);
        if !skip {
            tokens.push(token);
        }
    }
    let first = tokens.first()?;
    let second = tokens.get(1)?;
    if first.kind() == SyntaxKind::Identifier && second.kind() == SyntaxKind::ColonEqual {
        return Some(first.text().to_owned());
    }
    None
}

fn element_of(host: &SyntaxNode) -> Option<SyntaxNode> {
    if host.kind() == SyntaxKind::Element {
        return Some(host.clone());
    }
    host.child_node(SyntaxKind::Element)
}

fn inner_host(sub: &SyntaxNode) -> Option<SyntaxNode> {
    if sub.kind() == SyntaxKind::RepeatedElement || sub.kind() == SyntaxKind::ConditionalElement {
        return sub.child_node(SyntaxKind::SubElement);
    }
    Some(sub.clone())
}

fn subs(el: &SyntaxNode) -> Vec<SyntaxNode> {
    el.children()
        .filter(|n| {
            matches!(
                n.kind(),
                SyntaxKind::SubElement
                    | SyntaxKind::RepeatedElement
                    | SyntaxKind::ConditionalElement
            )
        })
        .collect()
}

fn component_of(root: &SyntaxNode) -> Option<SyntaxNode> {
    let mut all = vec![];
    for child in root.children() {
        if child.kind() == SyntaxKind::Component {
            all.push(child);
            continue;
        }
        if child.kind() == SyntaxKind::ExportsList {
            for nested in child.children() {
                if nested.kind() == SyntaxKind::Component {
                    all.push(nested);
                }
            }
        }
    }
    let window = all.iter().find(|c| {
        c.child_node(SyntaxKind::Element)
            .is_some_and(|el| base_name(&el) == "Window")
    });
    window.cloned().or_else(|| all.into_iter().next())
}

fn comment_markup(source: &str) -> Option<String> {
    let mut lines = source.lines();
    let header = lines.find(|line| !line.trim().is_empty())?;
    let rest = header.trim().strip_prefix(MARKER)?;
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let mut markup = vec![];
    for line in lines {
        let line = line.trim_start();
        if let Some(body) = line.strip_prefix("// ") {
            markup.push(body);
        } else if line == "//" {
            markup.push("");
        } else {
            break;
        }
    }
    Some(markup.join("\n"))
}

fn note_edits(root: &Node, markup: &str, options: &ImportOptions<'_>, losses: &mut Losses) {
    let types = token_types(options.tokens);
    let parsed = parse(
        markup,
        &ParseOptions {
            catalog: Some(options.catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    let Some(comment) = parsed.document else {
        return;
    };
    if has_errors(&parsed.diagnostics) {
        return;
    }
    let mut by_id = HashMap::new();
    index_text(&comment.root, &mut by_id);
    diff_text(root, "", &by_id, losses);
}

fn index_text(node: &Node, out: &mut HashMap<String, String>) {
    if let Some(id) = &node.id
        && let Some(text) = shown(node)
    {
        out.insert(id.clone(), text);
    }
    for list in node.slots.values() {
        for child in list {
            if let Child::Node(child) = child {
                index_text(child, out);
            }
        }
    }
    for child in &node.children {
        if let Child::Node(child) = child {
            index_text(child, out);
        }
    }
}

fn diff_text(node: &Node, parent: &str, comment: &HashMap<String, String>, losses: &mut Losses) {
    let path = path_of(parent, node);
    if let Some(id) = &node.id
        && let Some(new) = shown(node)
        && let Some(old) = comment.get(id)
        && new != *old
    {
        let note = format!("the file's text {new:?} differs from the source comment {old:?}");
        losses.push(LossKind::Text, &path, note);
    }
    for child in &node.children {
        if let Child::Node(child) = child {
            diff_text(child, &path, comment, losses);
        }
    }
}
