//! Slint that `import_slint` rejects, from the syntax tree (SPEC §9, "From Slint").
//! Parsed, never compiled or run. A brace that would read as a binding is escaped
//! the way the other importers escape it.

use std::collections::HashMap;

use i_slint_compiler::diagnostics::BuildDiagnostics;
use i_slint_compiler::literals::unescape_string;
use i_slint_compiler::parser::{self, NodeOrToken, SyntaxKind, SyntaxNode};
use weft_catalog::token_types;
use weft_core::{
    Catalog, Child, Code, Diagnostic, Document, Mode, Node, ParseOptions, Value, WEFT_VERSION,
    has_errors, is_id, is_loop_variable, parse,
};
use weft_import::{
    ImportResult, LossKind, Losses, MAX_DEPTH, MAX_NODES, empty_result, limit_reached, literal,
};

use crate::generate::MARKER;
use crate::import::{ImportOptions, MAX_SOURCE_LENGTH};

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
    // A model, an alert and a combobox share a Slint element with image, stack and select.
    let comment_kinds = comment_markup(source)
        .as_deref()
        .map(|markup| comment_kind_ids(markup, options))
        .unwrap_or_default();
    let mut reader = Reader::new(options.catalog, comment_kinds);
    if built.has_errors() {
        reader.lose(
            LossKind::Structure,
            "/",
            "syntax errors; parts were skipped",
        );
    }
    let root = reader.screen(&element);
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
    losses: Losses,
    ids: weft_import::IdState,
    nodes: usize,
    limited: bool,
    /// The repeated element of a list is an item only for that list's own children.
    in_list: bool,
    /// A `RadioButton` is a segment only inside the horizontal group.
    segments: bool,
    /// Ids the source comment calls `model`, `alert` or `combobox`.
    comment_kinds: HashMap<String, String>,
}

struct Fact {
    name: String,
    raw: String,
    string: Option<String>,
}

impl<'a> Reader<'a> {
    fn new(catalog: &'a Catalog, comment_kinds: HashMap<String, String>) -> Self {
        Reader {
            catalog,
            losses: Losses::default(),
            ids: weft_import::IdState::default(),
            nodes: 0,
            limited: false,
            in_list: false,
            segments: false,
            comment_kinds,
        }
    }

    fn lose(&mut self, kind: LossKind, path: &str, note: &str) {
        self.losses.push(kind, path, note);
    }

    fn screen(&mut self, window: &SyntaxNode) -> Node {
        self.nodes += 1;
        let mut root = Node::new("screen");
        let items = subs(window);
        let (explicit, body) = if base_name(window) == "Window" || base_name(window).is_empty() {
            screen_body(&items)
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
                out.extend(self.repeated(sub, parent, depth, self.in_list));
                continue;
            }
            if sub.kind() == SyntaxKind::ConditionalElement {
                self.lose(LossKind::Hidden, parent, "an if is read as shown");
            }
            let host = inner_host(sub);
            let Some(host) = host else { continue };
            let el = element_of(&host);
            let Some(el) = el else { continue };
            let id = sub_id(&host);
            out.extend(self.one(&el, id, parent, depth, false));
        }
        out
    }

    fn one(
        &mut self,
        el: &SyntaxNode,
        id: Option<String>,
        parent: &str,
        depth: usize,
        as_item: bool,
    ) -> Vec<Child> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            self.limited = true;
            return vec![];
        }
        let base = base_name(el);
        let Some(fallback) = kind_of(&base) else {
            let note = format!("`{base}` has no Weft kind; its children are kept");
            self.lose(LossKind::Kinds, parent, &note);
            return self.kids(&subs(el), parent, depth);
        };
        let kind = self.refined(&base, fallback, el, id.as_deref(), as_item);
        self.nodes += 1;
        let hint = string_of(&facts(el), "text");
        let mut node = Node::new(kind);
        let stem = format!("{parent}/{kind}");
        node.id = Some(self.fresh(id.as_deref(), kind, &hint, &stem));
        let here = path_of(parent, &node);
        // Descendants are not list items or segments; the caller restores the flags.
        let saved_list = self.in_list;
        let saved_segments = self.segments;
        self.in_list = false;
        self.segments = false;
        let extra = self.fill(&mut node, &base, el, &here, depth);
        self.in_list = saved_list;
        self.segments = saved_segments;
        self.blank_label(&mut node, &here);
        let mut out = vec![Child::Node(Box::new(node))];
        out.extend(extra);
        out
    }

    fn repeated(
        &mut self,
        sub: &SyntaxNode,
        parent: &str,
        depth: usize,
        as_item: bool,
    ) -> Vec<Child> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            self.limited = true;
            return vec![];
        }
        self.nodes += 1;
        let mut node = Node::new("each");
        // The model expression stays a loss; the loop variable is a name, not an expression.
        if let Some(name) = loop_name(sub) {
            node.props.insert("as".into(), Value::String(name));
        }
        let stem = format!("{parent}/each");
        node.id = Some(self.fresh(None, "each", "", &stem));
        let here = path_of(parent, &node);
        if let Some(host) = inner_host(sub)
            && let Some(el) = element_of(&host)
        {
            node.children = self.one(&el, sub_id(&host), &here, depth + 1, as_item);
        }
        vec![Child::Node(Box::new(node))]
    }

    /// Several generator shapes share one element name.
    fn refined(
        &self,
        base: &str,
        fallback: &'static str,
        el: &SyntaxNode,
        id: Option<&str>,
        as_item: bool,
    ) -> &'static str {
        // An item is printed as Text, TouchArea or VerticalLayout, and `<each>` is transparent,
        // so leaving the wrapper as a stack would not be a child a list accepts.
        if as_item && matches!(base, "Text" | "TouchArea" | "VerticalLayout") {
            return "item";
        }
        if base == "RadioButton" && self.segments {
            return "segment";
        }
        // That one property is the only line that separates a segmented control from a radio group.
        if base == "RadioGroup" && is_horizontal(el) {
            return "segmented-control";
        }
        if base == "Image" && self.comment_kind(id, "model") {
            return "model";
        }
        // The id sits on the column when an empty branch is emitted beside the widget.
        if base == "VerticalLayout" && splice_anon(el, "ListView").is_some() {
            return "list";
        }
        if base == "VerticalLayout" && splice_anon(el, "StandardTableView").is_some() {
            return "table";
        }
        // Slint has no alert widget, and select and combobox are both a ComboBox.
        if base == "VerticalLayout" && self.comment_kind(id, "alert") {
            return "alert";
        }
        if base == "ComboBox" && self.comment_kind(id, "combobox") {
            return "combobox";
        }
        fallback
    }

    fn comment_kind(&self, id: Option<&str>, kind: &str) -> bool {
        id.is_some_and(|name| self.comment_kinds.get(name).map(String::as_str) == Some(kind))
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
        self.note_rest(&fs, el, path);
        match base {
            "Text" | "Button" | "RadioButton" => self.words(node, &fs, "text", path),
            "MenuItem" => self.words(node, &fs, "title", path),
            "LineEdit" | "TextEdit" => self.attr(node, "value", &fs, "text", path),
            "CheckBox" | "Switch" => self.attr(node, "label", &fs, "text", path),
            "ComboBox" => self.lose(LossKind::Values, path, "a ComboBox model is not read"),
            "TouchArea" => return self.link(node, el, path, depth),
            "GroupBox" | "Tab" | "Menu" | "RadioGroup" => {
                self.attr(node, "label", &fs, "title", path);
            }
            "TabWidget" | "StandardTableView" | "Image" => {
                self.attr(node, "label", &fs, "accessible-label", path);
            }
            _ => {}
        }
        if base == "TextEdit" {
            let multiline = Value::String("multiline".into());
            node.props.insert("type".into(), multiline);
        }
        if let Some(children) = self.placed(node, base, el, path, depth) {
            if base == "GridLayout" {
                let count = children.len().max(1) as f64;
                node.props.insert("columns".into(), Value::Number(count));
            }
            node.children = children;
        }
        vec![]
    }

    /// The generator's wrapper is not a Weft element.
    fn placed(
        &mut self,
        node: &mut Node,
        base: &str,
        el: &SyntaxNode,
        path: &str,
        depth: usize,
    ) -> Option<Vec<Child>> {
        let next = depth + 1;
        if base == "MenuBar"
            && subs(el).len() == 1
            && let Some(menu) = anon_widget(el, "Menu")
        {
            self.attr(node, "label", &facts(&menu), "title", path);
            return Some(self.kids(&subs(&menu), path, next));
        }
        if base == "Tab" {
            let body = only_plain_column(el).unwrap_or_else(|| subs(el));
            return Some(self.kids(&body, path, next));
        }
        if base == "PopupWindow" {
            let items = only_plain_column(el).unwrap_or_else(|| subs(el));
            return Some(self.dialog_body(node, &items, path, next));
        }
        let kind = node.kind.clone();
        if base == "VerticalLayout" && matches!(kind.as_str(), "list" | "table") {
            let widget = if kind == "list" {
                "ListView"
            } else {
                "StandardTableView"
            };
            if let Some(inner) = anon_widget(el, widget)
                && let Some(children) = splice_anon(el, widget)
            {
                self.attr(node, "label", &facts(&inner), "accessible-label", path);
                self.note_rest(&facts(&inner), &inner, path);
                return Some(self.descend(kind == "list", false, &children, path, next));
            }
        }
        let nest = base.ends_with("Layout")
            || base.ends_with("Box")
            || matches!(
                base,
                "ListView" | "StandardTableView" | "TabWidget" | "Menu" | "MenuBar" | "RadioGroup"
            );
        if !nest {
            return None;
        }
        Some(self.descend(
            base == "ListView",
            kind == "segmented-control",
            &subs(el),
            path,
            next,
        ))
    }

    fn dialog_body(
        &mut self,
        node: &mut Node,
        items: &[SyntaxNode],
        path: &str,
        depth: usize,
    ) -> Vec<Child> {
        let Some(first) = items.first() else {
            return vec![];
        };
        if let Some(text) = caption_text(first) {
            self.attr(node, "label", &facts(&text), "text", path);
            return self.kids(&items[1..], path, depth);
        }
        self.kids(items, path, depth)
    }

    fn descend(
        &mut self,
        list: bool,
        segments: bool,
        items: &[SyntaxNode],
        parent: &str,
        depth: usize,
    ) -> Vec<Child> {
        let saved_list = self.in_list;
        let saved_segments = self.segments;
        if list {
            self.in_list = true;
        }
        if segments {
            self.segments = true;
        }
        let children = self.kids(items, parent, depth);
        self.in_list = saved_list;
        self.segments = saved_segments;
        children
    }

    fn note_rest(&mut self, fs: &[Fact], el: &SyntaxNode, path: &str) {
        if fs.iter().any(|f| f.name == "spacing") {
            self.lose(LossKind::Tokens, path, "spacing is not a dimension token");
        }
        if fs.iter().any(|f| f.name == "font-size") {
            self.lose(LossKind::Layout, path, "font-size is not a heading level");
        }
        if el
            .children()
            .any(|n| n.kind() == SyntaxKind::CallbackConnection)
        {
            self.lose(LossKind::Actions, path, "a callback is not kept");
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
        match &fact.string {
            Some(text) => {
                let shown = literal(&mut self.losses, path, text);
                node.children.push(Child::Text(shown));
            }
            None => self.lose(LossKind::Bindings, path, "the text expression is not kept"),
        }
    }

    fn attr(&mut self, node: &mut Node, prop: &str, facts: &[Fact], name: &str, path: &str) {
        let Some(fact) = named(facts, name) else {
            return;
        };
        match &fact.string {
            Some(text) => {
                let shown = literal(&mut self.losses, path, text);
                node.props.insert(prop.into(), Value::String(shown));
            }
            None => {
                let note = format!("`{name}` is not a literal");
                self.lose(LossKind::Bindings, path, &note);
            }
        }
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
        "ListView" => "list",
        "StandardTableView" => "table",
        "TabWidget" => "tabs",
        "Tab" => "tab",
        "PopupWindow" => "dialog",
        "MenuBar" | "Menu" => "menu",
        "MenuItem" => "menu-item",
        "Image" => "image",
        "RadioGroup" => "radio-group",
        "RadioButton" => "radio",
        _ => return None,
    })
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

fn screen_body(items: &[SyntaxNode]) -> (Option<String>, Vec<SyntaxNode>) {
    if let Some(unwrapped) = plain_box(items) {
        return unwrapped;
    }
    // A MenuBar cannot sit in a layout, so the generator places it beside the screen box.
    let rest: Vec<SyntaxNode> = items
        .iter()
        .filter(|sub| !is_widget(sub, "MenuBar"))
        .cloned()
        .collect();
    if rest.len() == items.len() {
        return (None, items.to_vec());
    }
    let Some((id, inner)) = plain_box(&rest) else {
        return (None, items.to_vec());
    };
    let mut body = Vec::new();
    let mut opened = false;
    for sub in items {
        if is_widget(sub, "MenuBar") {
            body.push(sub.clone());
        } else if !opened {
            body.extend(inner.iter().cloned());
            opened = true;
        }
    }
    (id, body)
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
        out.push(Fact { name, raw, string });
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

fn is_widget(sub: &SyntaxNode, widget: &str) -> bool {
    sub.kind() == SyntaxKind::SubElement
        && element_in(sub).is_some_and(|el| base_name(&el) == widget)
}

fn element_in(sub: &SyntaxNode) -> Option<SyntaxNode> {
    element_of(&inner_host(sub)?)
}

fn loop_name(repeated: &SyntaxNode) -> Option<String> {
    let declared = repeated.child_node(SyntaxKind::DeclaredIdentifier)?;
    let name = first_ident(&declared)?;
    is_loop_variable(&name).then_some(name)
}

fn is_horizontal(el: &SyntaxNode) -> bool {
    fact(el, "orientation").is_some_and(|f| f.raw == "Orientation.horizontal")
}

/// The list or table id is on the surrounding column, not on this widget.
fn splice_anon(el: &SyntaxNode, widget: &str) -> Option<Vec<SyntaxNode>> {
    let mut found = false;
    let mut out = Vec::new();
    for sub in subs(el) {
        let inner = element_in(&sub);
        let anon = sub.kind() == SyntaxKind::SubElement
            && sub_id(&sub).is_none()
            && inner
                .as_ref()
                .is_some_and(|inner| base_name(inner) == widget);
        if anon {
            if found {
                return None;
            }
            found = true;
            if let Some(inner) = inner {
                out.extend(subs(&inner));
            }
        } else {
            out.push(sub);
        }
    }
    found.then_some(out)
}

fn anon_widget(el: &SyntaxNode, widget: &str) -> Option<SyntaxNode> {
    let mut found = None;
    for sub in subs(el) {
        if sub.kind() != SyntaxKind::SubElement || sub_id(&sub).is_some() {
            continue;
        }
        let Some(inner) = element_in(&sub) else {
            continue;
        };
        if base_name(&inner) != widget {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(inner);
    }
    found
}

/// A tab or dialog body is wrapped in a column that is not its own element.
fn only_plain_column(el: &SyntaxNode) -> Option<Vec<SyntaxNode>> {
    let items = subs(el);
    let [only] = items.as_slice() else {
        return None;
    };
    if only.kind() != SyntaxKind::SubElement || sub_id(only).is_some() {
        return None;
    }
    let inner = element_in(only)?;
    if base_name(&inner) != "VerticalLayout" || !facts(&inner).is_empty() {
        return None;
    }
    let calls = inner
        .children()
        .any(|n| n.kind() == SyntaxKind::CallbackConnection);
    if calls {
        return None;
    }
    Some(subs(&inner))
}

fn caption_text(sub: &SyntaxNode) -> Option<SyntaxNode> {
    if sub.kind() != SyntaxKind::SubElement || sub_id(sub).is_some() {
        return None;
    }
    let el = element_in(sub)?;
    if base_name(&el) != "Text" {
        return None;
    }
    let fs = facts(&el);
    let [only] = fs.as_slice() else {
        return None;
    };
    (only.name == "text" && only.string.is_some()).then_some(el)
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

/// Kinds that share one Slint element: `model` with `image`, `alert` with `stack`, `combobox` with `select`.
fn comment_kind_ids(markup: &str, options: &ImportOptions<'_>) -> HashMap<String, String> {
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
    let Some(document) = parsed.document else {
        return HashMap::new();
    };
    if has_errors(&parsed.diagnostics) {
        return HashMap::new();
    }
    let mut ids = HashMap::new();
    collect_comment_kinds(&document.root, &mut ids);
    ids
}

fn collect_comment_kinds(node: &Node, ids: &mut HashMap<String, String>) {
    if matches!(node.kind.as_str(), "model" | "alert" | "combobox")
        && let Some(id) = &node.id
    {
        ids.insert(id.clone(), node.kind.clone());
    }
    for list in node.slots.values() {
        for child in list {
            if let Child::Node(child) = child {
                collect_comment_kinds(child, ids);
            }
        }
    }
    for child in &node.children {
        if let Child::Node(child) = child {
            collect_comment_kinds(child, ids);
        }
    }
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
