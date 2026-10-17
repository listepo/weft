//! The wrappers `generate::surface` writes, read back onto the element they hold: a `Border`
//! whose `Background` is a `Weft.` resource is the `material` of its stack or grid, with that
//! element's `padding`, `max-width` and `hidden`; a `Border` with a `PerspectiveTransform3D` is
//! the `perspective` of its element; and a `CompositeTransform3D` is the tilt's rotations.

use weft_core::{Child, Node, js_number};
use weft_import::{LossKind, element_path};

use super::{Placed, Reader, elements, is_named};
use crate::generate::surface::TURNS;
use crate::read::Xml;

/// The element a generated surface or perspective `Border` holds: its one element besides its
/// property elements, when it has a `Weft.` background or a 3D transform.
fn held<'a, 'i>(el: Xml<'a, 'i>) -> Option<Xml<'a, 'i>> {
    if !is_named(el, "Border") {
        return None;
    }
    let weft = el
        .attribute("Background")
        .is_some_and(|b| b.contains(" Weft."));
    let (properties, content): (Vec<_>, Vec<_>) =
        elements(el).partition(|c| c.tag_name().name().contains('.'));
    let turned = properties
        .iter()
        .any(|p| p.tag_name().name() == "Border.Transform3D");
    match content[..] {
        [one] if weft || turned => Some(one),
        _ => None,
    }
}

impl Reader<'_> {
    /// Reads a surface or perspective `Border` as the element it holds; false when `el` is not
    /// one.
    pub(super) fn wrapped(
        &mut self,
        el: Xml<'_, '_>,
        parent: &str,
        parent_kind: &str,
        depth: usize,
        out: &mut Placed,
    ) -> bool {
        let Some(inner) = held(el) else {
            return false;
        };
        let mut placed = Placed::default();
        self.element(inner, parent, parent_kind, depth + 1, &mut placed);
        match &mut placed.children[..] {
            [Child::Node(node)] => {
                let here = element_path(parent, node);
                self.attributes(el, node, &here);
                self.turns(el, node, &here);
            }
            _ => {
                let note = "a `Border` around what is not one element: its background and transform are not read";
                self.lose(LossKind::Layout, parent, note);
            }
        }
        out.children.extend(placed.children);
        for (name, list) in placed.slots {
            out.slots.entry(name).or_default().extend(list);
        }
        true
    }

    /// The 3D transform of `el` as the tilt of `node`: a `PerspectiveTransform3D`'s `Depth` is
    /// `perspective`, a `CompositeTransform3D`'s rotations the `rotate-*` props, with the sign
    /// CSS gives them.
    pub(super) fn turns(&mut self, el: Xml<'_, '_>, node: &mut Node, here: &str) {
        let tag = el.tag_name().name();
        let holder = elements(el).find(|c| {
            c.tag_name()
                .name()
                .strip_prefix(tag)
                .is_some_and(|p| p == ".Transform3D")
        });
        for transform in holder.into_iter().flat_map(elements) {
            let kind = transform.tag_name().name();
            for a in transform.attributes() {
                let (name, value) = (a.name(), a.value());
                let prop = match (kind, name) {
                    ("PerspectiveTransform3D", "Depth") => Some(("perspective", false)),
                    ("CompositeTransform3D", _) => TURNS
                        .iter()
                        .find(|(_, attr)| *attr == name)
                        .map(|(prop, _)| (*prop, true)),
                    _ => None,
                };
                let number = value.trim().parse::<f64>().ok().filter(|n| n.is_finite());
                match (prop, number) {
                    (Some((prop, negated)), Some(n)) => {
                        let n = if negated { -n } else { n };
                        self.set(node, prop, &js_number(n), here, name);
                    }
                    _ => {
                        let note = format!("`{name}` of a `{kind}` has no Weft form");
                        self.lose(LossKind::Layout, here, note);
                    }
                }
            }
        }
    }
}
