//! The layout vocabulary of the XAML (docs/winui-design.md, "Layout vocabulary"): a `Grid` drawn
//! for a justified or growing `stack` (tracks on one axis, a star track for a growing child, empty
//! star tracks for the free space) reads back as the stack's `direction`, `grow` and `justify`,
//! and the children's shared cross-axis alignment as its `align`. A `grid`'s own tracks read as
//! its `columns` alone when they are the ones the generator writes.

use roxmltree::Node as Xml;
use weft_core::{Child, Node, Value};
use weft_import::LossKind;

use super::{Placed, Reader, elements, is_named};

/// The tracks of a `Grid` read as a stack: its axis, and whether each track is star-sized.
pub struct Tracks {
    row: bool,
    star: Vec<bool>,
}

impl Tracks {
    /// The attribute that places a child along the stack.
    pub fn along(&self) -> &'static str {
        if self.row { "Grid.Column" } else { "Grid.Row" }
    }
}

/// The child elements, without property elements such as `Grid.ColumnDefinitions`.
fn children<'a, 'i>(el: Xml<'a, 'i>) -> impl Iterator<Item = Xml<'a, 'i>> {
    elements(el).filter(|c| !c.tag_name().name().contains('.'))
}

fn track(child: Xml<'_, '_>, attr: &str) -> Option<usize> {
    child.attribute(attr).map_or(Some(0), |v| v.parse().ok())
}

/// `Some` when `el` is a `Grid` drawn for a stack: tracks on one axis only, each `Auto` or `*`,
/// every child alone in one of them. Columns that are all star are a `grid`'s, so a row needs
/// an `Auto` track.
pub fn stack_tracks(el: Xml<'_, '_>) -> Option<Tracks> {
    if !is_named(el, "Grid") {
        return None;
    }
    let defs = |name: &str| elements(el).find(|c| c.tag_name().name() == name);
    let (row, list) = match (defs("Grid.ColumnDefinitions"), defs("Grid.RowDefinitions")) {
        (Some(columns), None) => (true, columns),
        (None, Some(rows)) => (false, rows),
        _ => return None,
    };
    let size = if row { "Width" } else { "Height" };
    let star = elements(list)
        .map(|t| match t.attribute(size) {
            None | Some("*" | "1*") => Some(true),
            Some("Auto") => Some(false),
            Some(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if row && star.iter().all(|s| *s) {
        return None;
    }
    let tracks = Tracks { row, star };
    let across = if row { "Grid.Row" } else { "Grid.Column" };
    let mut held = vec![false; tracks.star.len()];
    let fits = children(el).all(|c| {
        c.attribute(across).is_none_or(|v| v == "0")
            && !c.has_attribute("Grid.RowSpan")
            && !c.has_attribute("Grid.ColumnSpan")
            && track(c, tracks.along())
                .and_then(|t| held.get_mut(t))
                .is_some_and(|h| !std::mem::replace(h, true))
    });
    fits.then_some(tracks)
}

/// Whether `el` is a `grid` as the generator draws it: star columns over `Auto` rows, as many
/// rows as its children fill, each child alone in the next cell in document order. Its tracks
/// and placements then say nothing beyond `columns`.
pub fn filled(el: Xml<'_, '_>) -> bool {
    let defs = |name: &str| elements(el).find(|c| c.tag_name().name() == name);
    let sizes = |list: Option<Xml<'_, '_>>, attr: &str| -> Vec<String> {
        list.map(|l| {
            elements(l)
                .map(|t| t.attribute(attr).unwrap_or("*").to_owned())
                .collect()
        })
        .unwrap_or_default()
    };
    let columns = sizes(defs("Grid.ColumnDefinitions"), "Width");
    let rows = sizes(defs("Grid.RowDefinitions"), "Height");
    let kids: Vec<_> = children(el).collect();
    if columns.is_empty() || columns.iter().any(|c| !matches!(c.as_str(), "*" | "1*")) {
        return false;
    }
    let width = columns.len();
    rows.len() == kids.len().div_ceil(width)
        && rows.iter().all(|r| r == "Auto")
        && kids.iter().enumerate().all(|(i, c)| {
            !c.has_attribute("Grid.RowSpan")
                && !c.has_attribute("Grid.ColumnSpan")
                && track(*c, "Grid.Row") == Some(i / width)
                && track(*c, "Grid.Column") == Some(i % width)
        })
}

impl Reader<'_> {
    /// The children of a `Grid` read as a stack, in track order: a child in a star track grows,
    /// and the empty star tracks are the `justify` that put them there.
    pub(super) fn tracks(
        &mut self,
        (el, tracks): (Xml<'_, '_>, &Tracks),
        node: &mut Node,
        here: &str,
        depth: usize,
        placed: &mut Placed,
    ) {
        let mut kids: Vec<_> = children(el)
            .map(|c| (track(c, tracks.along()).unwrap_or_default(), c))
            .collect();
        kids.sort_by_key(|(t, _)| *t);
        let mut held = vec![false; tracks.star.len()];
        let mut grow = false;
        for (t, child) in kids {
            held[t] = true;
            let before = placed.children.len();
            self.element(child, here, "stack", depth + 1, placed);
            if !tracks.star[t] {
                continue;
            }
            grow = true;
            if let [Child::Node(n)] = &mut placed.children[before..] {
                n.props.insert("grow".into(), Value::Bool(true));
            } else {
                let note = "a star track that holds no single element: no `grow` is read";
                self.lose(LossKind::Layout, here, note);
            }
        }
        // `C` a held track, `S` an empty star track (free space), `A` an empty `Auto` track.
        let pattern: String = held
            .iter()
            .zip(&tracks.star)
            .map(|(held, star)| match (held, star) {
                (false, true) => 'S',
                (false, false) => 'A',
                (true, _) => 'C',
            })
            .collect();
        let inner = pattern.trim_matches('S');
        let plain = !inner.contains(['S', 'A']);
        let justify = match (pattern.starts_with('S'), pattern.ends_with('S')) {
            _ if !held.contains(&true) => Some("start"),
            (false, false) if plain => Some("start"),
            // SPEC §5.1: with a growing child `justify` has no effect, so nothing is spare.
            _ if grow => None,
            (true, false) if plain => Some("end"),
            (true, true) if plain => Some("center"),
            (false, false) if inner.split('S').all(|c| c == "C") => Some("space-between"),
            _ => None,
        };
        match justify {
            Some("start") => {}
            Some(j) => {
                node.props.insert("justify".into(), Value::String(j.into()));
            }
            None => {
                let note = format!("the tracks `{pattern}` are not a stack's `justify` and `grow`");
                self.lose(LossKind::Layout, here, note);
            }
        }
    }

    /// The children's shared cross-axis alignment as the stack's `align`; a row's `Center` is
    /// the default, which the generator writes out. Returns the attribute this reads for the
    /// children, so they do not report it.
    pub(super) fn align(&mut self, el: Xml<'_, '_>, node: &mut Node) -> Option<&'static str> {
        let row = node.props.get("direction") == Some(&Value::String("row".into()));
        let (attr, words) = if row {
            ("VerticalAlignment", ["Top", "Center", "Bottom", "Stretch"])
        } else {
            (
                "HorizontalAlignment",
                ["Left", "Center", "Right", "Stretch"],
            )
        };
        let mut values = children(el).map(|c| c.attribute(attr));
        let first = values.next().flatten()?;
        let at = words.iter().position(|w| *w == first)?;
        if !values.all(|v| v == Some(first)) {
            return None;
        }
        let align = ["start", "center", "end", "stretch"][at];
        if !(row && align == "center") {
            node.props
                .insert("align".into(), Value::String(align.into()));
        }
        Some(attr)
    }
}
