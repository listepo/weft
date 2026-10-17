//! What is drawn around an element rather than in it (SPEC §9, To WinUI): a `material` is a
//! `Border` whose `Background` is the token's `AcrylicBrush` and which holds the padding and the
//! width of the stack or grid, so the surface is the element's box; the 3D tilt is a
//! `CompositeTransform3D` on the element, or on its surface, under a `Border` carrying the
//! `PerspectiveTransform3D`, since WinUI projects an element through an ancestor's perspective
//! and gives each element one 3D transform.

use weft_core::{Node, Value, js_number};

use super::Gen;
use super::repetition::holder;
use crate::xaml::Element;

/// The tilt props and the `CompositeTransform3D` rotations they are. CSS turns clockwise and
/// WinUI counter-clockwise about the same axes, so each angle changes sign.
pub(crate) const TURNS: [(&str, &str); 3] = [
    ("rotate-x", "RotationX"),
    ("rotate-y", "RotationY"),
    ("rotate-z", "RotationZ"),
];

/// Kinds a WinUI control holds as its own item type (or the page holds as its content), where no
/// `Border` can stand around them.
const UNWRAPPED: &[&str] = &[
    "screen",
    "option",
    "radio",
    "segment",
    "menu-item",
    "item",
    "column",
    "row",
    "cell",
    "tab",
    "dialog",
];

impl Gen<'_> {
    /// `e` with its material surface and its tilt, whichever the node has. The parent sets its
    /// layout attributes on what this returns, and `Visibility` moves out with each wrapper, so
    /// a hidden element leaves no empty `Border` behind.
    pub(super) fn surface(&mut self, mut e: Element, node: &Node) -> Element {
        let material = match node.props.get("material") {
            None => None,
            Some(Value::Token(path)) => Some(path.clone()),
            Some(_) => {
                self.refuse(node, "a `material` that is not a token");
                None
            }
        };
        let number = |prop: &str| match node.props.get(prop) {
            Some(Value::Number(n)) => Some(*n),
            _ => None,
        };
        let turns: Vec<(&str, f64)> = TURNS
            .iter()
            .filter_map(|(prop, attr)| number(prop).map(|n| (*attr, n)))
            .collect();
        let perspective = number("perspective");
        if (!turns.is_empty() || perspective.is_some()) && UNWRAPPED.contains(&node.kind.as_str()) {
            self.later(node, &format!("a tilt on a `{}`", node.kind));
            return e;
        }
        if let Some(path) = material {
            let mut border = Element::new("Border");
            let brush = self.token(node, "ThemeResource", &path, "");
            border.attr("Background", brush);
            for attr in ["Padding", "MaxWidth", "Visibility"] {
                if let Some(value) = e.take(attr) {
                    border.attr(attr, value);
                }
            }
            border.children.push(e);
            e = border;
        }
        if !turns.is_empty() {
            let mut turn = Element::new("CompositeTransform3D");
            for (attr, degrees) in turns {
                turn.attr(attr, js_number(-degrees));
            }
            // The centre of a `CompositeTransform3D` is in pixels; the helper keeps it at the
            // middle of the element as its size changes.
            e.attr("weft:Tilt.Centered", "True");
            // A property element goes before the content, which must be contiguous.
            let property = format!("{}.Transform3D", e.name);
            e.children.insert(0, holder(property, turn));
        }
        if let Some(depth) = perspective {
            let mut view = Element::new("PerspectiveTransform3D");
            view.attr("Depth", js_number(depth));
            let mut border = Element::new("Border");
            if let Some(value) = e.take("Visibility") {
                border.attr("Visibility", value);
            }
            border.children.push(holder("Border.Transform3D", view));
            border.children.push(e);
            e = border;
        }
        e
    }
}
