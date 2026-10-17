//! The containers of SPEC §9, "To WinUI": `section`, `grid`, `dialog`, `list` and `item`, `table`
//! with its columns, rows and cells, and `tabs`. Each places its own children and slots.

use weft_core::{Child, Node, Value};

use super::Gen;
use crate::data::Leaf;
use crate::xaml::{Element, literal};

/// The containers, in the shape of `generate::KINDS`.
pub(super) const KINDS: &[(&str, &str, &str)] = &[
    ("section", "StackPanel", ""),
    (
        "grid",
        "Grid",
        "columns gap padding max-width min-column-width material",
    ),
    ("dialog", "ContentDialog", "open modal"),
    ("list", "ListView", "ordered"),
    ("item", "ListViewItem", "text"),
    ("table", "StackPanel", ""),
    ("column", "Button", "text sort"),
    ("row", "Grid", "selected"),
    ("cell", "ContentControl", "text"),
    ("tabs", "TabView", "selected"),
    ("tab", "TabViewItem", ""),
];

/// The named slots the containers draw.
pub(super) const SLOTS: &[(&str, &str)] = &[
    ("section", "header"),
    ("dialog", "actions"),
    ("list", "empty"),
    ("table", "empty"),
];

/// A section's card: the theme's card brushes, with literal sizes since Weft gives it none.
pub(crate) const CARD: &[(&str, &str)] = &[
    (
        "Background",
        "{ThemeResource CardBackgroundFillColorDefaultBrush}",
    ),
    ("BorderBrush", "{ThemeResource CardStrokeColorDefaultBrush}"),
    ("BorderThickness", "1"),
    ("CornerRadius", "8"),
    ("Padding", "16"),
    ("Spacing", "12"),
];

/// A grid's star columns become `ColumnDefinition`s, so a document cannot ask for millions.
const MAX_COLUMNS: usize = 100;

/// Panels and buttons carry no table semantics, so these name them for UI Automation and tell
/// the reader which panel is which.
fn control_type(kind: &str) -> Option<&'static str> {
    match kind {
        "table" => Some("table"),
        "row" => Some("row"),
        "cell" => Some("cell"),
        "column" => Some("column header"),
        _ => None,
    }
}

impl Gen<'_> {
    pub(super) fn container(&mut self, e: &mut Element, node: &Node) {
        if let Some(name) = control_type(&node.kind) {
            e.attr("AutomationProperties.LocalizedControlType", name);
        }
        match node.kind.as_str() {
            "section" => {
                e.attr("AutomationProperties.LandmarkType", "Custom");
                e.attr("AutomationProperties.LocalizedLandmarkType", "region");
                for (attr, value) in CARD {
                    e.attr(*attr, *value);
                }
                if let Some(header) = node.slots.get("header") {
                    e.children.push(self.slot("header", header, false));
                }
                self.children(e, &node.children);
            }
            "grid" => self.grid(e, node),
            "dialog" => {
                if let Some(open) = self.value(node, "open", Leaf::Flag, true) {
                    e.attr("weft:Dialog.IsOpen", open);
                }
                self.marker(e, node, "modal", "Modal");
                let mut content = self.panel(&node.children);
                if let Some(actions) = node.slots.get("actions") {
                    content.children.push(self.slot("actions", actions, true));
                }
                e.children.push(content);
            }
            "list" => {
                e.attr("SelectionMode", "None");
                self.marker(e, node, "ordered", "Ordered");
                self.list(e, node);
            }
            "item" | "cell" => self.mixed(e, node),
            "table" => self.table(e, node),
            "column" => {
                self.content(e, node, "Content");
                self.marker(e, node, "sort", "Sort");
            }
            "row" => {
                self.marker(e, node, "selected", "Selected");
                let cells = self.placed(nodes(&node.children));
                let tracks = self.columns.max(cells.len()).max(1);
                e.children.push(tracks_of("Column", "*", tracks));
                e.children
                    .extend(cells.into_iter().enumerate().map(|(i, c)| place(c, 0, i)));
            }
            "tabs" => {
                e.attr("IsAddTabButtonVisible", "False");
                if let Some(selected) = self.value(node, "selected", Leaf::Text, true) {
                    e.attr("weft:Choice.Value", selected);
                }
                self.children(e, &node.children);
            }
            _ => {
                e.attr("IsClosable", "False");
                // `Choice.Value` finds the selected tab by its `Tag`.
                if let Some(id) = &node.id {
                    e.attr("Tag", literal(id));
                }
                let content = self.panel(&node.children);
                e.children.push(content);
            }
        }
    }

    /// A slot's elements in a panel marked with its name, which nothing else in the XAML states.
    pub(super) fn slot(&mut self, name: &str, children: &[Child], row: bool) -> Element {
        let mut panel = Element::new("StackPanel");
        if row {
            panel.attr("Orientation", "Horizontal");
        }
        panel.attr("weft:Slot.Name", name);
        self.children(&mut panel, children);
        panel
    }

    /// A plain panel holding mixed content: the elements, and each text run as a wrapping
    /// `TextBlock` without an id. The reader reads it as its parent's content.
    pub(super) fn panel(&mut self, children: &[Child]) -> Element {
        let mut panel = Element::new("StackPanel");
        for child in children {
            match child {
                Child::Node(n) => {
                    let c = self.child(n, Some(&panel));
                    panel.children.extend(c);
                }
                Child::Text(t) if t.trim().is_empty() => {}
                Child::Text(t) => {
                    let mut run = Element::new("TextBlock");
                    run.attr("TextWrapping", "Wrap");
                    run.attr("Text", literal(t));
                    panel.children.push(run);
                }
            }
        }
        panel
    }

    /// The content of an `item` or a `cell`: its text alone, or a panel.
    fn mixed(&mut self, e: &mut Element, node: &Node) {
        if node.children.iter().any(|c| matches!(c, Child::Node(_))) {
            let content = self.panel(&node.children);
            e.children.push(content);
        } else {
            self.content(e, node, "Content");
        }
    }

    /// The generated elements of `nodes`, each to be placed in a track.
    pub(super) fn placed<'n>(&mut self, nodes: impl Iterator<Item = &'n Node>) -> Vec<Element> {
        nodes.filter_map(|n| self.node(n)).collect()
    }

    fn grid(&mut self, e: &mut Element, node: &Node) {
        let columns = match node.props.get("columns") {
            Some(Value::Number(n))
                if (1.0..=MAX_COLUMNS as f64).contains(n) && n.fract() == 0.0 =>
            {
                *n as usize
            }
            _ => {
                let why = format!("`columns` must be a literal from 1 to {MAX_COLUMNS}");
                self.refuse(node, why);
                1
            }
        };
        self.frame(e, node);
        if node.props.contains_key("min-column-width") {
            return self.columns_panel(e, node, columns);
        }
        self.gap(e, node, &["RowSpacing", "ColumnSpacing"]);
        let cells = self.placed(nodes(&node.children));
        e.children.push(tracks_of("Column", "*", columns));
        if !cells.is_empty() {
            e.children
                .push(tracks_of("Row", "Auto", cells.len().div_ceil(columns)));
        }
        let placed = cells.into_iter().enumerate();
        e.children
            .extend(placed.map(|(i, c)| place(c, i / columns, i % columns)));
    }

    /// The columns in a header `Grid`, then the rows, each a `Grid` with as many star tracks.
    fn table(&mut self, e: &mut Element, node: &Node) {
        let (columns, rows): (Vec<&Node>, Vec<&Node>) =
            nodes(&node.children).partition(|n| n.kind == "column");
        let outer = std::mem::replace(&mut self.columns, columns.len());
        let headers = self.placed(columns.into_iter());
        if !headers.is_empty() {
            let mut header = Element::new("Grid");
            header
                .children
                .push(tracks_of("Column", "*", headers.len()));
            header
                .children
                .extend(headers.into_iter().enumerate().map(|(i, c)| place(c, 0, i)));
            e.children.push(header);
        }
        for row in rows {
            // A repeated column would need a track per item of the header.
            if row.kind == "each" && nodes(&row.children).any(|n| n.kind != "row") {
                self.later(row, "an `<each>` of columns");
                continue;
            }
            let row = self.child(row, Some(e));
            e.children.extend(row);
        }
        self.empty(e, node, None);
        self.columns = outer;
    }
}

pub(super) fn nodes(children: &[Child]) -> impl Iterator<Item = &Node> {
    children.iter().filter_map(|c| match c {
        Child::Node(n) => Some(&**n),
        Child::Text(_) => None,
    })
}

/// `<Grid.ColumnDefinitions>` (or rows) of `count` tracks of `size`.
fn tracks_of(axis: &str, size: &str, count: usize) -> Element {
    tracks(axis, &vec![size; count])
}

/// The track definitions of one axis of a `Grid`, one per size.
pub(super) fn tracks(axis: &str, sizes: &[&str]) -> Element {
    let (list, track, attr) = match axis {
        "Column" => ("Grid.ColumnDefinitions", "ColumnDefinition", "Width"),
        _ => ("Grid.RowDefinitions", "RowDefinition", "Height"),
    };
    let mut defs = Element::new(list);
    for size in sizes {
        let mut t = Element::new(track);
        t.attr(attr, *size);
        defs.children.push(t);
    }
    defs
}

/// `e` in the given cell of its `Grid`; 0 is the default and is not written.
fn place(mut e: Element, row: usize, column: usize) -> Element {
    if row > 0 {
        e.attr("Grid.Row", row.to_string());
    }
    if column > 0 {
        e.attr("Grid.Column", column.to_string());
    }
    e
}
