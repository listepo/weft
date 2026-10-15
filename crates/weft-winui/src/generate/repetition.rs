//! Repetition (SPEC §9, "To WinUI"): an `<each>` as an `ItemsRepeater` whose template is typed by
//! the generated item class, a `list` of one `<each>` as a `ListView` bound to the list, and the
//! `empty` slots of `list` and `table`.

use weft_core::{Node, Value};

use super::containers::nodes;
use super::{Gen, path_of};
use crate::csharp::class_base;
use crate::data::{Leaf, Read};
use crate::xaml::{Element, literal};

impl Gen<'_> {
    /// A child of `parent`: an `<each>` repeats in the parent's flow, which only a `StackPanel`
    /// has; anything else is the element of its kind.
    pub(super) fn child(&mut self, node: &Node, parent: Option<&Element>) -> Option<Element> {
        if node.kind != "each" {
            return self.node(node);
        }
        let Some(parent) = parent.filter(|p| p.name == "StackPanel") else {
            // A grid's tracks, a row's cells, tabs, menus and choices cannot hold a repeater.
            self.later(node, "an `<each>` here");
            return None;
        };
        let (source, class) = self.enter(node)?;
        let mut e = Element::new("ItemsRepeater");
        e.attr("ItemsSource", source);
        markers(&mut e, node);
        let flow = ["Orientation", "Spacing"].map(|a| parent.get(a).map(|v| (a, v.to_owned())));
        // The default layout is a vertical stack without spacing, as a plain panel.
        if flow.iter().any(Option::is_some) {
            let mut layout = Element::new("StackLayout");
            for (attr, value) in flow.iter().flatten() {
                layout.attr(*attr, value);
            }
            e.children.push(holder("ItemsRepeater.Layout", layout));
        }
        // A template has one root: several elements share a panel with the parent's flow.
        let mut root = Element::new("StackPanel");
        for (attr, value) in flow.iter().flatten() {
            root.attr(*attr, value);
        }
        self.children(&mut root, &node.children);
        let one = nodes(&node.children).count() == 1 && root.children.len() == 1;
        let root = match one {
            true => root.children.pop().unwrap_or(root),
            false => root,
        };
        e.children
            .push(holder("ItemsRepeater.ItemTemplate", template(&class, root)));
        self.data.leave();
        Some(e)
    }

    /// The `ItemsSource` binding and the item class of an `<each>`, whose loop the generator is
    /// inside until `data.leave`.
    fn enter(&mut self, node: &Node) -> Option<(String, String)> {
        let (Some(Value::Bind { bind, not: false }), Some(Value::String(name))) =
            (node.props.get("in"), node.props.get("as"))
        else {
            self.refuse(node, "`in` must be a binding and `as` a name");
            return None;
        };
        let (path, item) = self.data.list(bind, &path_of(node), &mut self.problems)?;
        self.data.enter(name, item, bind);
        let class = format!(
            "{}Item",
            class_base(&self.name, &self.data.shapes[item].path)
        );
        Some((format!("{{x:Bind {path}, Mode=OneWay}}"), class))
    }

    /// A `ListView`'s items: its static ones, or the `ItemsSource` of its one `<each>` of one
    /// `item`, which the template draws as a `ContentControl` the list's container holds.
    pub(super) fn list(&mut self, e: &mut Element, node: &Node) {
        let each = nodes(&node.children).find(|n| n.kind == "each");
        let Some(each) = each else {
            self.children(e, &node.children);
            return self.empty(e, node, Some("ListView.Footer"));
        };
        let items: Vec<&Node> = nodes(&each.children).collect();
        if nodes(&node.children).count() > 1 || !matches!(items[..], [n] if n.kind == "item") {
            self.later(
                each,
                "an `<each>` beside other items, or of more than one `item`,",
            );
        } else if let Some((source, class)) = self.enter(each) {
            e.attr("ItemsSource", source);
            markers(e, each);
            if let Some(mut root) = self.node(items[0]) {
                root.name = "ContentControl";
                root.attr("AutomationProperties.LocalizedControlType", "list item");
                e.children
                    .push(holder("ListView.ItemTemplate", template(&class, root)));
            }
            self.data.leave();
        }
        self.empty(e, node, Some("ListView.Footer"));
    }

    /// The `empty` slot of a list or table, last, in `place` or in the element itself, shown by
    /// SPEC §5.1's rule: when the `state` is `empty`, or nothing repeats and no static item or row
    /// is there. A table's columns are not its rows.
    pub(super) fn empty(&mut self, e: &mut Element, node: &Node, place: Option<&'static str>) {
        let Some(slot) = node.slots.get("empty") else {
            return;
        };
        let at = path_of(node);
        let mut panel = self.slot("empty", slot, false);
        let shown: Vec<&Node> = nodes(&node.children)
            .filter(|n| n.kind != "column")
            .collect();
        let lists: Vec<&Node> = shown.iter().copied().filter(|n| n.kind == "each").collect();
        let count = match lists[..] {
            _ if shown.len() > lists.len() => None,
            [] => Some(String::new()),
            [each] => match each.props.get("in") {
                Some(Value::Bind { bind, .. }) => self
                    .data
                    .list(bind, &path_of(each), &mut self.problems)
                    .map(|(path, _)| format!("{path}.Count")),
                _ => None,
            },
            _ => {
                self.later(node, "an `empty` slot over several `<each>`");
                None
            }
        };
        let state = match node.props.get("state") {
            Some(Value::String(s)) if s == "empty" => Some(None),
            Some(Value::Bind { bind, not: false }) => self
                .data
                .read(bind, Read::Like(Leaf::Text), &at, &mut self.problems)
                .map(Some),
            _ => None,
        };
        // Shown is the default, so only a slot that may hide states its `Visibility`.
        let visibility = match (state, count.as_deref()) {
            (Some(None), _) | (_, Some("")) => None,
            (None, None) => Some("Collapsed".to_owned()),
            (None, Some(count)) => {
                Some(format!("{{x:Bind weft:Weft.Empty({count}), Mode=OneWay}}"))
            }
            (Some(Some(state)), None) => Some(format!(
                "{{x:Bind weft:Weft.EmptyState({state}), Mode=OneWay}}"
            )),
            (Some(Some(state)), Some(count)) => Some(format!(
                "{{x:Bind weft:Weft.Empty({state}, {count}), Mode=OneWay}}"
            )),
        };
        if let Some(visibility) = visibility {
            panel.attr("Visibility", visibility);
        }
        match place {
            Some(place) => e.children.push(holder(place, panel)),
            None => e.children.push(panel),
        }
    }
}

/// `weft:Each.Id` and `weft:Each.As`, which only the importer reads.
fn markers(e: &mut Element, each: &Node) {
    if let Some(id) = &each.id {
        e.attr("weft:Each.Id", literal(id));
    }
    if let Some(Value::String(name)) = each.props.get("as") {
        e.attr("weft:Each.As", literal(name));
    }
}

/// A `DataTemplate` compiled against the item class, so x:Bind paths in it are its members.
fn template(class: &str, root: Element) -> Element {
    let mut t = Element::new("DataTemplate");
    t.attr("x:DataType", format!("weft:{class}"));
    t.children.push(root);
    t
}

/// A property element holding `child`.
fn holder(name: &'static str, child: Element) -> Element {
    let mut e = Element::new(name);
    e.children.push(child);
    e
}
