//! WinUI XAML that `import_winui` rejects, read element by element (SPEC §9, "From WinUI"). The
//! XAML is parsed as XML, never compiled or run: a DTD is refused, so no entity is expanded, and
//! the length, the XML nodes, the elements and their nesting are bounded. roxmltree parses an
//! element within its parent's call, so nesting is counted before parsing, and the parse runs on
//! a stack sized for that count (`on_reader_stack`). Each control of the
//! mapping is its kind; what maps to nothing is a loss at the Weft path of the element it is on,
//! and the result is repaired until it validates in lenient mode.

mod controls;

use roxmltree::{Node as Xml, ParsingOptions};
use weft_core::{
    Catalog, Child, Code, Content, Diagnostic, Document, Map, Node, PropType, Value, WEFT_VERSION,
    is_id, squash, universal_prop,
};
use weft_import::{
    IdState, ImportResult, LossKind, Losses, MAX_DEPTH, MAX_NODES, Scalar, coerce, element_path,
    empty_result, limit_reached, literal, on_reader_stack, repair,
};

use crate::import::{ImportOptions, MAX_SOURCE_LENGTH, MAX_XML_DEPTH, MAX_XML_NODES, nested_past};
use controls::{Conv, PRESENTATION, XAML};

/// Attributes of the design surface (`d:`, `mc:`): they never reach the running page.
const DESIGN_TIME: &[&str] = &[
    "http://schemas.microsoft.com/expression/blend/2008",
    "http://schemas.openxmlformats.org/markup-compatibility/2006",
];
/// The `Weft.cs` classes whose attached properties carry Weft meaning (design, Markers).
const HELPERS: &[&str] = &["Props.", "On.", "Slot.", "Each.", "Choice.", "Dialog."];

/// Reads `source`, a XAML page, user control, window or loose panel, without its source comment.
/// A page the generator wrote, unedited, belongs to `import_winui`.
pub fn read_winui(source: &str, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics = vec![];
    if source.len() > MAX_SOURCE_LENGTH {
        let what = format!("is longer than {MAX_SOURCE_LENGTH} bytes");
        limit_reached(&mut diagnostics, "#", &what);
        return empty_result(diagnostics);
    }
    if nested_past(source, MAX_XML_DEPTH) {
        let what = format!("nests XML elements more than {MAX_XML_DEPTH} levels deep");
        limit_reached(&mut diagnostics, "#", &what);
        return empty_result(diagnostics);
    }
    on_reader_stack("weft-import-winui", || read(source, options))
        .unwrap_or_else(|| unreadable(vec![], "The XAML reader stopped unexpectedly."))
}

fn read(source: &str, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics = vec![];
    let parsing = ParsingOptions {
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    let xml = match roxmltree::Document::parse_with_options(source, parsing) {
        Ok(xml) => xml,
        Err(roxmltree::Error::NodesLimitReached) => {
            let what = format!("has more than {MAX_XML_NODES} XML nodes");
            limit_reached(&mut diagnostics, "#", &what);
            return empty_result(diagnostics);
        }
        Err(e) => {
            let message = format!("The source is not well-formed XML without a DTD: {e}.");
            return unreadable(diagnostics, &message);
        }
    };
    let top = xml.root_element();
    if top.tag_name().namespace() != Some(PRESENTATION) {
        let message = "The root element is not in the XAML presentation namespace.";
        return unreadable(diagnostics, message);
    }
    let mut reader = Reader {
        catalog: options.catalog,
        losses: Losses::default(),
        ids: IdState::default(),
        elements: 0,
        limited: false,
    };
    // Ids the source states are claimed first, so a generated id never takes one.
    reader.ids.reserved = top
        .descendants()
        .filter_map(explicit_id)
        .filter(|id| is_id(id))
        .map(str::to_owned)
        .collect();
    let root = reader.screen(top);
    if reader.limited {
        limit_reached(&mut diagnostics, "#", "is past the import limit");
    }
    let mut result = ImportResult {
        document: Document {
            weft: WEFT_VERSION.into(),
            version: None,
            context: Vec::new(),
            fragments: Default::default(),
            root,
        },
        losses: reader.losses.0,
        diagnostics,
    };
    repair(&mut result, options.catalog);
    result
}

fn unreadable(mut diagnostics: Vec<Diagnostic>, message: &str) -> ImportResult {
    diagnostics.push(Diagnostic::new(
        Code::W601,
        "#",
        message,
        "WinUI XAML: an element of the presentation namespace, without a DTD",
    ));
    empty_result(diagnostics)
}

struct Reader<'a> {
    catalog: &'a Catalog,
    losses: Losses,
    ids: IdState,
    elements: usize,
    limited: bool,
}

/// Where the elements read under one parent go: its children, or one of its slots.
#[derive(Default)]
struct Placed {
    children: Vec<Child>,
    slots: Map<Vec<Child>>,
}

impl Reader<'_> {
    fn lose(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    /// The screen is the content of the page, under the `ScrollViewer` the generator puts around
    /// it; a vertical `StackPanel` there is the screen itself.
    fn screen(&mut self, top: Xml<'_, '_>) -> Node {
        let holder = matches!(top.tag_name().name(), "Page" | "UserControl" | "Window");
        let mut outer = vec![];
        let mut body = vec![top];
        if holder {
            outer.push(top);
            body = elements(top).collect();
        }
        let (properties, rest): (Vec<_>, Vec<_>) = body
            .into_iter()
            .partition(|el| el.tag_name().name().contains('.'));
        body = rest;
        while let [one] = body[..]
            && is_named(one, "ScrollViewer")
        {
            outer.push(one);
            body = elements(one).collect();
        }
        let panel = match body[..] {
            [one]
                if is_named(one, "StackPanel")
                    && controls::kind_of(one).is_some_and(|(kind, _)| kind == "stack")
                    && one.attribute("Orientation").is_none_or(|o| o == "Vertical") =>
            {
                Some(one)
            }
            _ => None,
        };
        let mut root = Node::new("screen");
        let hint = panel.map(name_hint).unwrap_or_default();
        root.id = Some(self.id(panel.and_then(explicit_id), "screen", &hint, ""));
        let here = element_path("", &root);
        for el in outer.into_iter().chain(panel) {
            self.attributes(el, &mut root, &here);
        }
        let mut placed = Placed::default();
        for el in properties {
            self.element(el, &here, "screen", 1, &mut placed);
        }
        match panel {
            Some(panel) => self.body(panel, &mut root, &here, 0),
            None => {
                for el in body {
                    self.element(el, &here, "screen", 1, &mut placed);
                }
            }
        }
        self.settle(&mut root, placed, &here);
        root
    }

    fn id(&mut self, explicit: Option<&str>, kind: &str, name: &str, parent: &str) -> String {
        if let Some(id) = explicit
            && is_id(id)
            && !self.ids.used.contains(id)
        {
            self.ids.used.insert(id.to_owned());
            self.ids.reserved.remove(id);
            return id.to_owned();
        }
        let note = match explicit {
            Some(id) => format!("the id {id:?} is not a valid Weft id, or is taken"),
            None => "an id was generated".to_owned(),
        };
        let id = self.ids.fresh(kind, name);
        self.lose(LossKind::Ids, &format!("{parent}/{kind}#{id}"), note);
        id
    }

    fn element(
        &mut self,
        el: Xml<'_, '_>,
        parent: &str,
        parent_kind: &str,
        depth: usize,
        out: &mut Placed,
    ) {
        if depth > MAX_DEPTH || self.elements >= MAX_NODES {
            self.limited = true;
            return;
        }
        let tag = el.tag_name().name();
        if let Some((owner, property)) = tag.split_once('.') {
            match property {
                "Content" | "Items" | "Flyout" | "ItemTemplate" => {
                    self.through(el, parent, parent_kind, depth, out);
                }
                // The grid counts its columns itself.
                "ColumnDefinitions" | "RowDefinitions" if owner == "Grid" => {}
                _ => self.lose(LossKind::Props, parent, format!("`{tag}` is not read")),
            }
            return;
        }
        if let Some(slot) = slot_name(el) {
            for a in el.attributes() {
                if !matches!(a.name(), "Orientation" | "Slot.Name") {
                    let (kind, what) = controls::loss_of(a.name(), a.value());
                    let note = format!("`{}` on the `{slot}` slot row: {what}", a.name());
                    self.lose(kind, parent, note);
                }
            }
            let mut inner = Placed::default();
            self.through(el, parent, parent_kind, depth, &mut inner);
            out.slots
                .entry(slot.to_owned())
                .or_default()
                .extend(inner.children);
            for (name, list) in inner.slots {
                out.slots.entry(name).or_default().extend(list);
            }
            return;
        }
        if let Some(loss) = controls::wrapper(el) {
            if let Some((kind, what)) = loss {
                self.lose(kind, parent, format!("`{tag}`: {what}"));
            }
            return self.through(el, parent, parent_kind, depth, out);
        }
        let Some((kind, structure)) = controls::kind_of(el) else {
            let note = format!("`{tag}` is not a Weft kind; its content is read in its place");
            self.lose(LossKind::Kinds, parent, note);
            return self.through(el, parent, parent_kind, depth, out);
        };
        self.elements += 1;
        let mut node = Node::new(kind);
        node.id = Some(self.id(explicit_id(el), kind, &name_hint(el), parent));
        let here = element_path(parent, &node);
        if let Some(what) = structure {
            self.lose(LossKind::Structure, &here, format!("`{tag}`: {what}"));
        }
        if tag == "PasswordBox" {
            node.props
                .insert("type".into(), Value::String("password".into()));
        }
        self.attributes(el, &mut node, &here);
        self.body(el, &mut node, &here, depth);
        out.children.push(Child::Node(Box::new(node)));
    }

    /// The elements inside `el`, read in its place.
    fn through(
        &mut self,
        el: Xml<'_, '_>,
        parent: &str,
        parent_kind: &str,
        depth: usize,
        out: &mut Placed,
    ) {
        for child in elements(el) {
            self.element(child, parent, parent_kind, depth + 1, out);
        }
    }

    fn attributes(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str) {
        let tag = el.tag_name().name();
        for a in el.attributes() {
            let (name, value) = (a.name(), a.value());
            let markup = value.starts_with('{') && !value.starts_with("{}");
            let helper = a
                .namespace()
                .is_some_and(|n| n.starts_with("using:"))
                .then(|| HELPERS.iter().find(|h| name.starts_with(**h)))
                .flatten();
            match a.namespace() {
                // `x:Name` stands in for a missing `AutomationId`; `x:Class` names the code-behind.
                Some(XAML) if matches!(name, "Name" | "Class") => continue,
                Some(ns) if DESIGN_TIME.contains(&ns) => continue,
                Some(_) if helper == Some(&"Props.") && !markup => {
                    let prop = controls::marker_prop(&name["Props.".len()..]);
                    self.set(node, &prop, value, here, name);
                    continue;
                }
                Some(_) if helper == Some(&"Props.") => {
                    let (kind, what) = controls::loss_of(name, value);
                    self.lose(kind, here, format!("`{name}` on `{tag}`: {what}"));
                    continue;
                }
                Some(_) if helper.is_some() => {
                    let kind = match helper {
                        Some(&"On.") => LossKind::Actions,
                        Some(&"Each.") => LossKind::Repetition,
                        _ => LossKind::Bindings,
                    };
                    self.lose(kind, here, format!("`{name}` on `{tag}` is not read yet"));
                    continue;
                }
                Some(_) => {
                    let note = format!("`{name}` on `{tag}`: an attribute of another namespace");
                    self.lose(LossKind::Props, here, note);
                    continue;
                }
                None => {}
            }
            if name == "AutomationProperties.AutomationId" || controls::expected(&node.kind, name) {
                continue;
            }
            if let Some((_, _, prop, conv)) = controls::attr_of(&node.kind, name) {
                let raw = match conv {
                    Conv::Same if !markup => Some(value),
                    Conv::Not => match value {
                        "True" => Some("false"),
                        "False" => Some("true"),
                        _ => None,
                    },
                    Conv::Words(words) => words.iter().find(|(w, _)| *w == value).map(|w| w.1),
                    Conv::Same => None,
                };
                if let Some(raw) = raw {
                    self.set(node, prop, raw, here, name);
                    continue;
                }
                if !markup {
                    let note = format!("`{name}=\"{value}\"` on `{tag}` has no Weft value");
                    self.lose(LossKind::Values, here, note);
                    continue;
                }
            }
            let (kind, what) = controls::loss_of(name, value);
            self.lose(kind, here, format!("`{name}` on `{tag}`: {what}"));
        }
    }

    /// `raw` as `prop` of `node`, typed by the catalog; an empty `prop` is the element's text.
    fn set(&mut self, node: &mut Node, prop: &str, raw: &str, here: &str, attr: &str) {
        // `{}` only keeps XAML from reading a value as a markup extension.
        let raw = raw.strip_prefix("{}").unwrap_or(raw);
        if prop.is_empty() {
            if !raw.is_empty() {
                let text = literal(&mut self.losses, here, raw);
                node.children.push(Child::Text(text));
            }
            return;
        }
        // `HeadingLevel` states the level; levels 4 to 6 share one ramp style.
        if attr == "Style" && node.props.contains_key(prop) {
            return;
        }
        let component = self.catalog.components.get(&node.kind);
        // The values of `state` are the states of the kind, not of the prop.
        if prop == "state"
            && component
                .and_then(|c| c.states.as_ref())
                .is_some_and(|s| s.iter().any(|v| v == raw))
        {
            node.props
                .insert(prop.to_owned(), Value::String(raw.to_owned()));
            return;
        }
        let def = component
            .and_then(|c| c.prop(prop))
            .or_else(|| universal_prop(prop));
        let Some(def) = def else {
            let note = format!("`{attr}`: a `{}` has no `{prop}`", node.kind);
            return self.lose(LossKind::Props, here, note);
        };
        let scalar = match (def.kind, raw) {
            (PropType::Boolean, "True") => Scalar::Bool(true),
            (PropType::Boolean, "False") => Scalar::Bool(false),
            _ => Scalar::String(raw.to_owned()),
        };
        let value = match coerce(def, &scalar) {
            Some(Value::String(s)) => Value::String(literal(&mut self.losses, here, &s)),
            Some(v) => v,
            None => {
                let note = format!("`{attr}=\"{raw}\"` is not a value of `{prop}`");
                return self.lose(LossKind::Values, here, note);
            }
        };
        node.props.insert(prop.to_owned(), value);
    }

    /// What is inside the element: child elements for a container, text for the others.
    fn body(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str, depth: usize) {
        let content = self.catalog.components.get(&node.kind).map(|d| &d.content);
        if matches!(content, None | Some(Content::Nodes | Content::Mixed)) {
            let mixed = matches!(content, Some(Content::Mixed));
            let kind = node.kind.clone();
            let mut placed = Placed::default();
            for child in el.children() {
                if child.is_element() {
                    self.element(child, here, &kind, depth + 1, &mut placed);
                    continue;
                }
                let text = squash(child.text().filter(|_| child.is_text()).unwrap_or_default());
                if text.is_empty() {
                } else if mixed {
                    placed
                        .children
                        .push(Child::Text(literal(&mut self.losses, here, &text)));
                } else {
                    let note = format!("text directly inside a `{}` is not read", node.kind);
                    self.lose(LossKind::Text, here, note);
                }
            }
            if node.kind == "grid" {
                self.columns(el, node, here);
            }
            return self.settle(node, placed, here);
        }
        let mut text = String::new();
        self.inline(el, &node.kind, here, &mut text, depth);
        let text = squash(&text);
        if text.is_empty() {
            return;
        }
        let none = matches!(content, Some(Content::None));
        if (none && node.props.contains_key("label")) || (!none && !node.children.is_empty()) {
            let note = format!(
                "the `{}` already has its text; inline text is not read",
                node.kind
            );
            return self.lose(LossKind::Text, here, note);
        }
        let text = literal(&mut self.losses, here, &text);
        if none {
            node.props.insert("label".into(), Value::String(text));
        } else {
            node.children.push(Child::Text(text));
        }
    }

    /// The text an element shows through inline content: runs, spans and a plain `TextBlock`.
    fn inline(&mut self, el: Xml<'_, '_>, kind: &str, here: &str, out: &mut String, depth: usize) {
        if depth > MAX_DEPTH {
            self.limited = true;
            return;
        }
        for child in el.children() {
            if child.is_text() {
                out.push_str(child.text().unwrap_or_default());
                continue;
            }
            if !child.is_element() {
                continue;
            }
            let tag = child.tag_name().name();
            match tag {
                "Run" | "TextBlock" => {
                    out.push_str(child.attribute("Text").unwrap_or_default());
                    self.inline(child, kind, here, out, depth + 1);
                }
                "LineBreak" => out.push(' '),
                "Span" | "Bold" | "Italic" | "Underline" | "Hyperlink" => {
                    let note = format!("`{tag}` formatting is dropped; its text is kept");
                    self.lose(LossKind::Text, here, note);
                    self.inline(child, kind, here, out, depth + 1);
                }
                _ => {
                    let note = format!("`{tag}` inside a `{kind}` is not read");
                    self.lose(LossKind::Structure, here, note);
                }
            }
        }
    }

    /// A `Grid`'s columns are its `ColumnDefinition`s; their widths and the rows are layout.
    fn columns(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str) {
        let mut columns = 0;
        for tracks in elements(el).filter(|c| c.tag_name().name().starts_with("Grid.")) {
            if tracks.tag_name().name() == "Grid.ColumnDefinitions" {
                columns += elements(tracks).count();
            }
            let note = format!(
                "`{}`: the track sizes are not read",
                tracks.tag_name().name()
            );
            self.lose(LossKind::Layout, here, note);
        }
        let columns = columns.max(1).to_string();
        self.set(node, "columns", &columns, here, "Grid.ColumnDefinitions");
    }

    fn settle(&mut self, node: &mut Node, placed: Placed, here: &str) {
        node.children.extend(placed.children);
        let def = self.catalog.components.get(&node.kind);
        for (name, list) in placed.slots {
            if def.is_some_and(|d| d.slot(&name).is_some()) {
                node.slots.entry(name).or_default().extend(list);
            } else {
                let note = format!(
                    "a `{}` has no `{name}` slot; its elements are children",
                    node.kind
                );
                self.lose(LossKind::Slots, here, note);
                node.children.extend(list);
            }
        }
    }
}

fn elements<'a, 'i>(el: Xml<'a, 'i>) -> impl Iterator<Item = Xml<'a, 'i>> {
    el.children().filter(Xml::is_element)
}

fn is_named(el: Xml<'_, '_>, name: &str) -> bool {
    el.tag_name().namespace() == Some(PRESENTATION) && el.tag_name().name() == name
}

fn explicit_id<'a>(el: Xml<'a, '_>) -> Option<&'a str> {
    el.attribute("AutomationProperties.AutomationId")
        .or_else(|| el.attribute((XAML, "Name")))
}

fn slot_name<'a>(el: Xml<'a, '_>) -> Option<&'a str> {
    el.attributes()
        .find(|a| a.namespace().is_some_and(|n| n.starts_with("using:")) && a.name() == "Slot.Name")
        .map(|a| a.value())
}

/// The text a generated id is named after: the first literal name, header, content or text.
fn name_hint(el: Xml<'_, '_>) -> String {
    ["AutomationProperties.Name", "Header", "Content", "Text"]
        .iter()
        .filter_map(|a| el.attribute(*a))
        .find(|v| !v.starts_with('{'))
        .unwrap_or_default()
        .to_owned()
}
