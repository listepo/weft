//! The layout vocabulary (SPEC §5.1, docs/winui-design.md "Layout vocabulary"), written in the
//! forms `read::layout` reads back: a `stack` whose `justify` leaves free space or whose child
//! grows is a `Grid` of tracks along its direction, `align` is each child's cross-axis
//! alignment, `padding` and `max-width` are token resources, and `min-column-width` is a
//! `weft:ColumnsPanel`, or a `weft:ColumnsLayout` when the grid repeats one element.

use weft_core::{Node, Value};

use super::Gen;
use super::containers::{nodes, tracks};
use crate::xaml::Element;

impl Gen<'_> {
    pub(super) fn stack(&mut self, e: &mut Element, node: &Node) {
        let row = self.word(node, "direction") == Some("row");
        let justify = self.word(node, "justify").unwrap_or("start");
        let kids: Vec<&Node> = nodes(&node.children).collect();
        let grows: Vec<bool> = kids.iter().map(|k| self.grows(k)).collect();
        let grow = grows.contains(&true);
        // A single child has no space to share between children, so it sits at the start.
        let spare = justify != "start"
            && !kids.is_empty()
            && !(justify == "space-between" && kids.len() < 2);
        if row && !grows.is_empty() && grows.iter().all(|g| *g) {
            // Star columns alone are a `grid`'s tracks, so they would read back as one.
            self.later(node, "a row whose every child grows");
        }
        let gap = match (grow || spare, row) {
            (false, _) => "Spacing",
            (true, true) => "ColumnSpacing",
            (true, false) => "RowSpacing",
        };
        // A `StackPanel`'s flow is set before its children, since an `<each>` among them repeats
        // in it. A `StackPanel` is vertical by default, so only a stated `direction` is written.
        if !(grow || spare) && node.props.contains_key("direction") {
            e.attr("Orientation", if row { "Horizontal" } else { "Vertical" });
        }
        self.gap(e, node, &[gap]);
        let mut children = vec![];
        for kid in &kids {
            if kid.kind == "each" && nodes(&kid.children).any(|n| n.props.contains_key("grow")) {
                // A repeater is one track, so its items cannot each take the free space.
                self.later(kid, "`grow` inside an `<each>`");
            }
            let parent = if grow || spare { None } else { Some(&*e) };
            if let Some(child) = self.child(kid, parent) {
                children.push(child);
            }
        }
        self.frame(e, node);
        self.marker(e, node, "wrap", "Wrap");
        if justify != "start" && (grow || !spare) {
            // SPEC §5.1: a single child, or a growing one, leaves no space to share, so the
            // `justify` draws nothing and is kept as a marker.
            self.marker(e, node, "justify", "Justify");
        }
        let align = self.word(node, "align");
        let across = match (align, row) {
            (None, false) => None,
            (Some(a), false) => Some((
                "HorizontalAlignment",
                ["Left", "Center", "Right", "Stretch"],
                a,
            )),
            // SPEC §5.1: a row centres its children unless it says otherwise.
            (a, true) => Some((
                "VerticalAlignment",
                ["Top", "Center", "Bottom", "Stretch"],
                a.unwrap_or("center"),
            )),
        };
        if let Some((attr, words, align)) = across {
            let at = ["start", "center", "end", "stretch"]
                .iter()
                .position(|w| *w == align);
            for child in &mut children {
                child.attr(attr, words[at.unwrap_or(1)]);
            }
        }
        if !(grow || spare) {
            e.children = children;
            return;
        }
        e.name = "Grid".into();
        // Empty star tracks take the free space where `justify` puts it; a growing child takes
        // it all (SPEC §5.1), so then there are none.
        let spacer = |at: &str| !grow && justify == at;
        let mut sizes = vec![];
        let mut at = vec![];
        if spacer("end") || spacer("center") {
            sizes.push("*");
        }
        for (i, grows) in grows.iter().enumerate() {
            if i > 0 && spacer("space-between") {
                sizes.push("*");
            }
            at.push(sizes.len());
            sizes.push(if *grows { "*" } else { "Auto" });
        }
        if spacer("center") {
            sizes.push("*");
        }
        let (axis, along) = if row {
            ("Column", "Grid.Column")
        } else {
            ("Row", "Grid.Row")
        };
        e.children.push(tracks(axis, &sizes));
        for (mut child, at) in children.into_iter().zip(at) {
            if at > 0 {
                child.attr(along, at.to_string());
            }
            e.children.push(child);
        }
    }

    /// Whether a stack's child grows: a literal `grow`, since the tracks are fixed in the XAML.
    fn grows(&mut self, kid: &Node) -> bool {
        match kid.props.get("grow") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => {
                self.refuse(kid, "a bound `grow`");
                false
            }
        }
    }

    /// `padding` and `max-width` of a `stack` or `grid`, from their token resources.
    pub(super) fn frame(&mut self, e: &mut Element, node: &Node) {
        for (prop, attr, suffix) in [
            ("padding", "Padding", ".thickness"),
            ("max-width", "MaxWidth", ""),
        ] {
            match node.props.get(prop) {
                None => {}
                Some(Value::Token(path)) => {
                    let value = self.token(node, "StaticResource", path, suffix);
                    e.attr(attr, value);
                }
                Some(_) => self.refuse(node, format!("a `{prop}` that is not a token")),
            }
        }
    }

    /// A `grid` with `min-column-width`: a `weft:ColumnsPanel` showing at most `columns`
    /// columns, each at least the token wide, its children in order. A grid whose only child is
    /// an `<each>` is an `ItemsRepeater` whose `weft:ColumnsLayout` has the same columns.
    pub(super) fn columns_panel(&mut self, e: &mut Element, node: &Node, columns: usize) {
        let kids: Vec<&Node> = nodes(&node.children).collect();
        let each = match kids[..] {
            [each] if each.kind == "each" => Some(each),
            _ => None,
        };
        let mut layout = Element::new("weft:ColumnsLayout");
        let columned = match each {
            Some(_) => &mut layout,
            None => {
                e.name = "weft:ColumnsPanel".into();
                &mut *e
            }
        };
        columned.attr("Columns", columns.to_string());
        match node.props.get("min-column-width") {
            Some(Value::Token(path)) => {
                let value = self.token(node, "StaticResource", path, "");
                columned.attr("MinColumnWidth", value);
            }
            _ => self.refuse(node, "a `min-column-width` that is not a token"),
        }
        self.gap(columned, node, &["Spacing"]);
        let Some(each) = each else {
            let cells = self.placed(kids.into_iter());
            e.children.extend(cells);
            return;
        };
        // An `ItemsRepeater` has no padding, so its layout keeps it around the columns.
        if let Some(padding) = e.take("Padding") {
            layout.attr("Padding", padding);
        }
        self.repeat(e, each, layout);
    }
}
