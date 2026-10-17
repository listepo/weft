//! WinUI XAML that `import_winui` rejects, read element by element (SPEC §9, "From WinUI"). The
//! XAML is parsed as XML, never compiled or run: a DTD is refused, so no entity is expanded, and
//! the length, the XML nodes, the elements and their nesting are bounded. roxmltree parses an
//! element within its parent's call, so nesting is counted before parsing, and the parse runs on
//! a stack sized for that count (`on_reader_stack`). Each control of the
//! mapping is its kind; what maps to nothing is a loss at the Weft path of the element it is on,
//! and the result is repaired until it validates in lenient mode.

mod controls;
mod layout;
mod surface;
mod values;

use roxmltree::{Node as Xml, ParsingOptions};
use weft_core::{
    Catalog, Child, Code, Content, Diagnostic, Document, Map, Node, PropType, Value, WEFT_VERSION,
    is_binding, is_id, squash, universal_prop,
};
use weft_import::{
    IdState, ImportResult, LossKind, Losses, MAX_DEPTH, MAX_NODES, Scalar, coerce, element_path,
    empty_result, limit_reached, literal, on_reader_stack, repair,
};

use crate::import::{ImportOptions, MAX_SOURCE_LENGTH, MAX_XML_DEPTH, MAX_XML_NODES, nested_past};
use controls::{Conv, PRESENTATION, XAML};
use indexmap::IndexMap;
use weft_catalog::Token;

/// Attributes of the design surface (`d:`, `mc:`): they never reach the running page.
const DESIGN_TIME: &[&str] = &[
    "http://schemas.microsoft.com/expression/blend/2008",
    "http://schemas.openxmlformats.org/markup-compatibility/2006",
];
/// The `Weft.cs` classes whose attached properties carry Weft meaning (design, Markers).
const HELPERS: &[&str] = &[
    "Props.", "On.", "Slot.", "Each.", "Choice.", "Dialog.", "Tilt.",
];

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
        tokens: options.tokens,
        consumed: vec![],
        losses: Losses::default(),
        ids: IdState::default(),
        elements: 0,
        limited: false,
        loops: vec![],
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
    tokens: &'a IndexMap<String, Token>,
    /// Attributes of the current element's parent reads for its children (`align`, tracks).
    consumed: Vec<&'static str>,
    losses: Losses,
    ids: IdState,
    elements: usize,
    limited: bool,
    /// The loop variables of the `<each>` templates being read, outermost first.
    loops: Vec<String>,
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
            let each = el.parent_element().is_some_and(|p| self.repeats(p));
            match property {
                // A repeating list's template is its `<each>`, read with the list.
                "ItemTemplate" if each && owner == "ListView" => {}
                "Template" if each && owner == "Each" => {}
                "Template" if owner == "Each" => {
                    let note = "an `Each.Template` without `Each.As` and a bound `Each.Source` is not read";
                    self.lose(LossKind::Repetition, parent, note);
                }
                "Content" | "Items" | "Flyout" | "ItemTemplate" | "Footer" => {
                    self.through(el, parent, parent_kind, depth, out);
                }
                // The grid counts its columns itself; a repeater's stack follows its parent's.
                "ColumnDefinitions" | "RowDefinitions" if owner == "Grid" => {}
                "Layout" if owner == "ItemsRepeater" => {}
                // The page's own copy of the project's tokens, which the props name.
                "Resources" if owner == "Page" && own_tokens(el) => {}
                // The tilt, read with the element it turns.
                "Transform3D" => {}
                _ => self.lose(LossKind::Props, parent, format!("`{tag}` is not read")),
            }
            return;
        }
        if let Some(slot) = slot_name(el) {
            // An `empty` slot's `Visibility` is the rule of SPEC §5.1, not a prop.
            let derived = |name: &str| name == "Visibility" && slot == "empty";
            for a in el.attributes() {
                if !matches!(a.name(), "Orientation" | "Slot.Name") && !derived(a.name()) {
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
        // A text run of mixed content: the generator writes it without the AutomationId every
        // `text` of the source carries.
        let mixed = self.catalog.components.get(parent_kind).map(|d| &d.content);
        if tag == "TextBlock"
            && explicit_id(el).is_none()
            && matches!(mixed, Some(Content::Mixed))
            && let Some(text) = el.attribute("Text")
            && (!text.starts_with('{') || text.starts_with("{}"))
        {
            let text = squash(text.strip_prefix("{}").unwrap_or(text));
            if !text.is_empty() {
                let text = literal(&mut self.losses, parent, &text);
                out.children.push(Child::Text(text));
            }
            return;
        }
        if is_named(el, "ItemsRepeater")
            && let Some(layout) = columns_layout(el)
            && self.repeats(el)
        {
            return self.repeated_grid(el, layout, parent, depth, out);
        }
        if is_named(el, "ItemsRepeater")
            && let Some(each) = self.each(el, parent, parent_kind, depth)
        {
            out.children.push(Child::Node(Box::new(each)));
            return;
        }
        // A menu's items with an `<each>` among them; the flyout itself is no element.
        if is_named(el, "MenuFlyout") && self.repeats(el) {
            let mut inner = Placed::default();
            self.through(el, parent, parent_kind, depth, &mut inner);
            if let Some(each) = self.each(el, parent, parent_kind, depth) {
                place(&mut inner.children, el, each);
            }
            out.children.extend(inner.children);
            for (name, list) in inner.slots {
                out.slots.entry(name).or_default().extend(list);
            }
            return;
        }
        if self.wrapped(el, parent, parent_kind, depth, out) {
            return;
        }
        if let Some(loss) = controls::wrapper(el) {
            if let Some((kind, what)) = loss {
                self.lose(kind, parent, format!("`{tag}`: {what}"));
            }
            return self.through(el, parent, parent_kind, depth, out);
        }
        let Some((mut kind, structure)) = controls::kind_of(el) else {
            let note = format!("`{tag}` is not a Weft kind; its content is read in its place");
            self.lose(LossKind::Kinds, parent, note);
            return self.through(el, parent, parent_kind, depth, out);
        };
        if kind == "grid" && layout::stack_tracks(el).is_some() {
            kind = "stack";
        }
        self.elements += 1;
        let mut node = Node::new(kind);
        node.id = Some(self.id(explicit_id(el), kind, &name_hint(el), parent));
        let here = element_path(parent, &node);
        if let Some(what) = structure {
            self.lose(LossKind::Structure, &here, format!("`{tag}`: {what}"));
        }
        // The generator's date and time row is one date picker.
        let row = kind == "date-picker" && tag == "StackPanel";
        let typed = match tag {
            "PasswordBox" => Some("password"),
            "TimePicker" => Some("time"),
            _ if row => Some("datetime"),
            _ => None,
        };
        if let Some(typed) = typed {
            node.props
                .insert("type".into(), Value::String(typed.into()));
        }
        self.attributes(el, &mut node, &here);
        self.turns(el, &mut node, &here);
        if row {
            // The row's pickers carry its label, `disabled` and bounds; they draw its value.
            for child in elements(el) {
                let part = child.tag_name().name();
                if matches!(part, "CalendarDatePicker" | "TimePicker") {
                    self.attributes(child, &mut node, &here);
                } else {
                    let note = format!("`{part}` in a date and time row is not read");
                    self.lose(LossKind::Structure, &here, note);
                }
            }
        } else if kind != "color-picker" || tag != "DropDownButton" {
            // A colour button's swatch and flyout only draw the value `Choice.Value` holds.
            self.body(el, &mut node, &here, depth);
        }
        if matches!(tag, "ListView" | "ComboBox")
            && let Some(each) = self.each(el, &here, kind, depth)
        {
            place(&mut node.children, el, each);
        }
        out.children.push(Child::Node(Box::new(node)));
    }

    /// A `grid` with `min-column-width` whose only child is an `<each>`: the repeater carries
    /// the grid's own attributes and its `weft:ColumnsLayout` the columns.
    fn repeated_grid(
        &mut self,
        el: Xml<'_, '_>,
        layout: Xml<'_, '_>,
        parent: &str,
        depth: usize,
        out: &mut Placed,
    ) {
        self.elements += 1;
        let mut node = Node::new("grid");
        node.id = Some(self.id(explicit_id(el), "grid", &name_hint(el), parent));
        let here = element_path(parent, &node);
        self.attributes(layout, &mut node, &here);
        self.attributes(el, &mut node, &here);
        self.turns(el, &mut node, &here);
        if let Some(each) = self.each(el, &here, "grid", depth) {
            node.children.push(Child::Node(Box::new(each)));
        }
        out.children.push(Child::Node(Box::new(node)));
    }

    /// Whether `el` draws an `<each>`: a loop variable and a list to repeat.
    fn repeats(&self, el: Xml<'_, '_>) -> bool {
        each_name(el).is_some() && self.repeated(el).is_some()
    }

    /// The path of the list `el` repeats: its `ItemsSource`, or the `Each.Source` of a choice or
    /// a menu, which cannot bind one.
    fn repeated(&self, el: Xml<'_, '_>) -> Option<String> {
        let list = el
            .attribute("ItemsSource")
            .or_else(|| helper(el, "Each.Source"));
        list.and_then(|v| self.list_path(el, v))
    }

    /// The `<each>` an `ItemsRepeater` or a `ListView` with `weft:Each.As` draws: its list from
    /// `ItemsSource`, its children from the template, whose own paths are the loop's item.
    fn each(
        &mut self,
        el: Xml<'_, '_>,
        parent: &str,
        parent_kind: &str,
        depth: usize,
    ) -> Option<Node> {
        let name = each_name(el)?;
        let path = self.repeated(el)?;
        let mut node = Node::new("each");
        node.id = Some(self.id(helper(el, "Each.Id"), "each", name, parent));
        node.props.insert(
            "in".into(),
            Value::Bind {
                bind: path,
                not: false,
            },
        );
        node.props
            .insert("as".into(), Value::String(name.to_owned()));
        let here = element_path(parent, &node);
        self.loops.push(name.to_owned());
        let mut placed = Placed::default();
        let templates = elements(el).filter(|t| {
            let name = t.tag_name().name();
            name.ends_with(".ItemTemplate") || name == "Each.Template"
        });
        for root in templates.flat_map(elements).flat_map(elements) {
            // The panel the generator gives a template of several elements has the parent's flow.
            let shared = is_named(root, "StackPanel")
                && root
                    .attributes()
                    .all(|a| matches!(a.name(), "Orientation" | "Spacing"));
            match shared {
                true => self.through(root, &here, parent_kind, depth + 1, &mut placed),
                false => self.element(root, &here, parent_kind, depth + 1, &mut placed),
            }
        }
        self.loops.pop();
        node.children = placed.children;
        Some(node)
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
        // What the parent reads from its children is not read from what a wrapper holds.
        let consumed = std::mem::take(&mut self.consumed);
        for child in elements(el) {
            self.element(child, parent, parent_kind, depth + 1, out);
        }
        self.consumed = consumed;
    }

    fn attributes(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str) {
        let tag = el.tag_name().name();
        // A `max-width` stretches its element, so the `Stretch` belongs to it.
        let max_width = el
            .attribute("MaxWidth")
            .is_some_and(|v| v.contains(" Weft."));
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
                // A marker holds its prop as written; `Choice.Value` holds a choice's `value` (a
                // tab list's `selected`), and `Dialog.IsOpen` a dialog's `open`.
                Some(_)
                    if helper == Some(&"Props.")
                        || matches!(name, "Choice.Value" | "Dialog.IsOpen") =>
                {
                    let prop = match name.strip_prefix("Props.") {
                        Some(marker) => controls::marker_prop(marker),
                        None if name == "Dialog.IsOpen" => "open".to_owned(),
                        None if tag == "TabView" => "selected".to_owned(),
                        None => "value".to_owned(),
                    };
                    match markup {
                        false => self.set(node, &prop, value, here, name),
                        true => self.bound(el, node, (name, &prop, &Conv::Same), value, here),
                    }
                    continue;
                }
                Some(_) if helper == Some(&"On.") => {
                    let event = controls::marker_prop(&name["On.".len()..]);
                    self.event(el, node, (name, &[event.as_str()]), value, here);
                    continue;
                }
                // The helper only keeps the tilt's centre in the middle of the element.
                Some(_) if name == "Tilt.Centered" => continue,
                // `<each>` reads its markers and list itself.
                Some(_) if helper == Some(&"Each.") && self.repeats(el) => continue,
                Some(_) if helper.is_some() => {
                    let kind = match helper {
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
            if name == "AutomationProperties.AutomationId"
                || (name == "ItemsSource" && self.repeats(el))
                || controls::expected(&node.kind, name, value)
                || self.consumed.contains(&name)
                || (name == "HorizontalAlignment" && value == "Stretch" && max_width)
            {
                continue;
            }
            if let Some(prop) = controls::token_of(&node.kind, name)
                && self.token(node, (name, prop), value, here)
            {
                continue;
            }
            if let Some(events) = controls::events_of(name)
                && value.starts_with("{x:Bind")
            {
                self.event(el, node, (name, events), value, here);
                continue;
            }
            // An editable ComboBox puts an item's content in its text box, so an option there
            // shows its value and keeps its text, if any, in the tooltip.
            let editable = el
                .ancestors()
                .find(|p| is_named(*p, "ComboBox"))
                .is_some_and(|p| p.attribute("IsEditable") == Some("True"));
            if tag == "ComboBoxItem"
                && name == "Content"
                && (editable || el.attribute("ToolTipService.ToolTip").is_some())
            {
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
                if value.starts_with("{x:Bind") {
                    self.bound(el, node, (name, prop, conv), value, here);
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
            let tracks = (kind == "stack")
                .then(|| layout::stack_tracks(el))
                .flatten();
            // A column is the default direction, so only a row of tracks states one.
            if tracks.as_ref().is_some_and(|t| t.along() == "Grid.Column") {
                node.props
                    .insert("direction".into(), Value::String("row".into()));
            }
            let align = (kind == "stack").then(|| self.align(el, node)).flatten();
            // A filled grid's placements follow from the document order.
            let filled = kind == "grid" && layout::filled(el);
            let consumed = align
                .into_iter()
                .chain(tracks.as_ref().map(layout::Tracks::along))
                .chain(
                    filled
                        .then_some(["Grid.Row", "Grid.Column"])
                        .into_iter()
                        .flatten(),
                );
            let outer = std::mem::replace(&mut self.consumed, consumed.collect());
            if let Some(t) = &tracks {
                self.tracks((el, t), node, here, depth, &mut placed);
            }
            for child in el.children().filter(|_| tracks.is_none()) {
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
            self.consumed = outer;
            // A `ColumnsPanel` states its `Columns`; a `Grid` has them as tracks.
            if node.kind == "grid" && is_named(el, "Grid") {
                self.columns(el, node, here, filled);
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
                // The tilt, read with the element it turns.
                _ if tag.strip_suffix(".Transform3D") == Some(el.tag_name().name()) => {}
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

    /// A `Grid`'s columns are its `ColumnDefinition`s; their widths and the rows are layout,
    /// unless they are the `filled` ones of a generated grid.
    fn columns(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str, filled: bool) {
        let mut columns = 0;
        for tracks in elements(el).filter(|c| c.tag_name().name().starts_with("Grid.")) {
            if tracks.tag_name().name() == "Grid.ColumnDefinitions" {
                columns += elements(tracks).count();
            }
            if filled {
                continue;
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

/// The generator's `weft:ColumnsLayout` of an `ItemsRepeater`.
fn columns_layout<'a, 'i>(el: Xml<'a, 'i>) -> Option<Xml<'a, 'i>> {
    let holder = elements(el).find(|c| is_named(*c, "ItemsRepeater.Layout"))?;
    elements(holder).find(|l| {
        l.tag_name().name() == "ColumnsLayout"
            && l.tag_name()
                .namespace()
                .is_some_and(|n| n.starts_with("using:"))
    })
}

/// Whether a `Page.Resources` holds only `Weft.` resources, in theme dictionaries or not: the
/// tokens a page generated with its own carries.
fn own_tokens(el: Xml<'_, '_>) -> bool {
    el.descendants().filter(|d| d.is_element()).all(|d| {
        let theme = d
            .parent_element()
            .is_some_and(|p| p.tag_name().name() == "ResourceDictionary.ThemeDictionaries");
        match d.attribute((XAML, "Key")) {
            Some(key) => key.starts_with("Weft.") || theme,
            None => matches!(
                d.tag_name().name(),
                "Page.Resources" | "ResourceDictionary" | "ResourceDictionary.ThemeDictionaries"
            ),
        }
    })
}

fn is_named(el: Xml<'_, '_>, name: &str) -> bool {
    el.tag_name().namespace() == Some(PRESENTATION) && el.tag_name().name() == name
}

fn explicit_id<'a>(el: Xml<'a, '_>) -> Option<&'a str> {
    el.attribute("AutomationProperties.AutomationId")
        .or_else(|| el.attribute((XAML, "Name")))
}

/// The value of the attached property `name` of a `Weft.cs` class, such as `Each.As`.
fn helper<'a>(el: Xml<'a, '_>, name: &str) -> Option<&'a str> {
    el.attributes()
        .find(|a| a.namespace().is_some_and(|n| n.starts_with("using:")) && a.name() == name)
        .map(|a| a.value())
}

/// The loop variable `weft:Each.As` names, when it is one.
fn each_name<'a>(el: Xml<'a, '_>) -> Option<&'a str> {
    helper(el, "Each.As").filter(|name| is_binding(&format!("${name}")))
}

fn slot_name<'a>(el: Xml<'a, '_>) -> Option<&'a str> {
    helper(el, "Slot.Name")
}

/// `each` among `children`, after the first `weft:Each.At` elements of `el` (0 when it states
/// none, the rest when it states more).
fn place(children: &mut Vec<Child>, el: Xml<'_, '_>, each: Node) {
    let at = helper(el, "Each.At")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    let index = children
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, Child::Node(_)))
        .nth(at)
        .map_or(children.len(), |(i, _)| i);
    children.insert(index, Child::Node(Box::new(each)));
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
