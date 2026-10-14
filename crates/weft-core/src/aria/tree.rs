//! Roles, names and states per kind, as `@weft/render-react` exposes them (`aria.ts`, `roles.ts`):
//! the tree a browser would build from the reference renderer's page.

use serde_json::Value as Json;

use super::AriaNode;
use super::inst::{Inst, InstChild, content_text};
use crate::{ARIA_ROLES, js_number, safe_url};

/// Roles whose accessible name is computed from their content when no label is given (ARIA 1.2).
const NAME_FROM_CONTENT: &[&str] = &[
    "button",
    "cell",
    "checkbox",
    "columnheader",
    "gridcell",
    "heading",
    "link",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "option",
    "radio",
    "row",
    "rowheader",
    "switch",
    "tab",
    "tooltip",
    "treeitem",
];

/// Roles that add nothing to the tree; their children take their place.
pub(super) const TRANSPARENT: &[&str] = &["none", "presentation", "generic"];

/// The radio group the radios being built belong to.
#[derive(Clone, Copy)]
struct Ctx<'t, 'a> {
    group: Option<&'t Inst<'a>>,
}

/// The name and states of one node; `name` replaces the computed name when set.
#[derive(Default)]
struct Extra {
    name: Option<String>,
    states: Vec<(&'static str, Json)>,
    url: Option<String>,
}

impl Extra {
    fn states(states: &[(&'static str, bool)]) -> Self {
        Extra {
            states: states.iter().map(|(k, v)| (*k, Json::Bool(*v))).collect(),
            ..Extra::default()
        }
    }

    fn disabled(n: &Inst<'_>) -> Self {
        Extra::states(&[("disabled", n.flag("disabled"))])
    }
}

pub(super) fn text_node(name: impl Into<String>) -> AriaNode {
    AriaNode {
        role: "text".into(),
        name: name.into(),
        ..AriaNode::default()
    }
}

/// The role of an element the renderer has no mapping for: an extension's declared `role`, the
/// catalog role of a kind known to the catalog but not to the renderer, or `group` (SPEC §8).
fn fallback_role<'a>(n: &Inst<'a>) -> &'a str {
    let declared = match n.def {
        Some(def) => Some(def.role.as_str()),
        None => n.literal("role"),
    };
    declared
        .filter(|r| ARIA_ROLES.contains(r))
        .unwrap_or("group")
}

/// `region` and `form` are landmarks only when named; unnamed, HTML-AAM maps them to `generic`.
fn exposed_role<'r>(role: &'r str, n: &Inst<'_>) -> &'r str {
    if matches!(role, "region" | "form") && n.label().is_empty() {
        "generic"
    } else {
        role
    }
}

fn node(role: &str, n: &Inst<'_>, children: Vec<AriaNode>, extra: Extra) -> Vec<AriaNode> {
    if TRANSPARENT.contains(&role) {
        return children;
    }
    let name = extra.name.unwrap_or_else(|| {
        let label = n.label();
        if label.is_empty() && NAME_FROM_CONTENT.contains(&role) {
            content_text(n.ordered())
        } else {
            label
        }
    });
    let states = extra
        .states
        .into_iter()
        .filter(|(_, v)| *v != Json::Bool(false))
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    vec![AriaNode {
        role: role.to_owned(),
        name,
        states,
        url: extra.url,
        children,
    }]
}

fn many<'t, 'a: 't>(
    list: impl IntoIterator<Item = &'t InstChild<'a>>,
    ctx: Ctx<'_, 'a>,
) -> Vec<AriaNode> {
    list.into_iter().flat_map(|c| build(c, ctx)).collect()
}

/// A text run that is left out when it is empty.
fn text_of(t: String) -> Vec<AriaNode> {
    if t.is_empty() {
        vec![]
    } else {
        vec![text_node(t)]
    }
}

fn build<'a>(c: &InstChild<'a>, ctx: Ctx<'_, 'a>) -> Vec<AriaNode> {
    match c {
        InstChild::Text(t) => vec![text_node(t.clone())],
        InstChild::Node(n) => build_node(n, ctx),
    }
}

fn build_node<'a>(n: &Inst<'a>, ctx: Ctx<'_, 'a>) -> Vec<AriaNode> {
    let kids = || many(n.ordered(), ctx);
    if n.def.is_none() {
        return node(
            exposed_role(fallback_role(n), n),
            n,
            kids(),
            Extra::default(),
        );
    }
    let plain = Extra::default;
    match n.kind {
        "screen" => node("main", n, kids(), plain()),
        "stack" | "grid" => kids(),
        "text" => text_of(content_text(&n.children)),
        "section" => node(exposed_role("region", n), n, kids(), plain()),
        "heading" => {
            let t = content_text(&n.children);
            let label = n.label();
            let extra = Extra {
                name: Some(if label.is_empty() { t.clone() } else { label }),
                states: vec![("level", Json::from(n.heading_level()))],
                url: None,
            };
            node("heading", n, text_of(t), extra)
        }
        // An empty alt marks an image decorative, which removes it from the tree.
        "image" | "model" if n.label().is_empty() => vec![],
        "image" | "model" => node("img", n, vec![], plain()),
        "link" => {
            let extra = Extra {
                url: safe_url(&n.text("href")),
                ..Extra::default()
            };
            node("link", n, vec![text_node(content_text(&n.children))], extra)
        }
        "button" | "menu-item" => {
            let role = if n.kind == "button" {
                "button"
            } else {
                "menuitem"
            };
            let content = vec![text_node(content_text(&n.children))];
            node(role, n, content, Extra::disabled(n))
        }
        "form" => node(exposed_role("form", n), n, kids(), plain()),
        "field" => {
            let disabled = n.flag("disabled");
            let extra = Extra::states(&[("disabled", disabled), ("invalid", n.field_invalid())]);
            let mut out = node(n.field_role(), n, text_of(n.field_value()), extra);
            out.extend(text_of(n.text("error")));
            out
        }
        "checkbox" | "switch" => {
            let states = [
                ("checked", n.flag("checked")),
                ("disabled", n.flag("disabled")),
            ];
            node(n.kind, n, vec![], Extra::states(&states))
        }
        "radio-group" | "segmented-control" => {
            let inner = many(n.ordered(), Ctx { group: Some(n) });
            node("radiogroup", n, inner, plain())
        }
        "radio" | "segment" => {
            let label = n.label();
            let checked = n.radio_checked(ctx.group);
            let extra = Extra {
                name: Some(if label.is_empty() {
                    content_text(&n.children)
                } else {
                    label
                }),
                ..Extra::states(&[("checked", checked), ("disabled", n.flag("disabled"))])
            };
            node("radio", n, vec![], extra)
        }
        "select" => {
            let selected = n.selected_option().map(|(o, _)| o);
            let options = n
                .options()
                .flat_map(|o| {
                    let on = selected.is_some_and(|s| std::ptr::eq(s, o));
                    let content = vec![text_node(content_text(&o.children))];
                    node("option", o, content, Extra::states(&[("selected", on)]))
                })
                .collect();
            node("combobox", n, options, Extra::disabled(n))
        }
        // The value is the text of the control, as for a field: a browser lists it after the name.
        "slider" => {
            let value = vec![text_node(js_number(n.slider_value()))];
            node("slider", n, value, Extra::disabled(n))
        }
        "stepper" => {
            let value = vec![text_node(js_number(n.stepper_value()))];
            node("spinbutton", n, value, Extra::disabled(n))
        }
        "date-picker" | "color-picker" => {
            let value = if n.kind == "color-picker" {
                n.color_value()
            } else {
                n.date_value()
            };
            node("textbox", n, text_of(value), Extra::disabled(n))
        }
        "combobox" => node("combobox", n, text_of(n.text("value")), Extra::disabled(n)),
        // Outside a select an option has no list to belong to; it is its text.
        "option" => vec![text_node(content_text(&n.children))],
        "list" => node("list", n, kids(), plain()),
        "item" => node("listitem", n, kids(), plain()),
        "table" => node("table", n, table_body(n, ctx), plain()),
        "tabs" => tabs_tree(n, ctx),
        // A tab outside `tabs` has no tab list; it is a named group of its content.
        "tab" => node("group", n, kids(), plain()),
        "dialog" if n.dialog_open() => node("dialog", n, kids(), plain()),
        "dialog" => vec![],
        "alert" => node("alert", n, kids(), plain()),
        "menu" => node("menu", n, kids(), plain()),
        // `row`, `column` and `cell` outside a table are containers with their catalog role.
        _ => node(exposed_role(fallback_role(n), n), n, kids(), plain()),
    }
}

fn is_kind(c: &InstChild<'_>, kind: &str) -> bool {
    c.node().is_some_and(|n| n.kind == kind)
}

/// As the renderer draws a table: the columns are the header row in one row group, everything
/// else goes to the body row group, and content that is not a row is wrapped in a row and a cell
/// (SPEC §5.1).
fn table_body<'a>(table: &Inst<'a>, ctx: Ctx<'_, 'a>) -> Vec<AriaNode> {
    let columns: Vec<&InstChild<'a>> = table
        .children
        .iter()
        .filter(|c| is_kind(c, "column"))
        .collect();
    let mut body = Vec::new();
    if table.shows_empty() {
        // The empty content shares one row and one cell, like any other non-row content.
        let empty: Vec<&InstChild<'a>> = table.slot("empty").unwrap_or_default().iter().collect();
        body.push(wrap_row(&empty, ctx));
    } else {
        for c in table.children.iter().filter(|c| !is_kind(c, "column")) {
            match c.node().filter(|r| r.kind == "row") {
                Some(row) => {
                    let states = Extra::states(&[("selected", row.flag("selected"))]);
                    body.extend(node("row", row, row_cells(row, ctx), states));
                }
                None => body.push(wrap_row(&[c], ctx)),
            }
        }
    }
    let mut groups = Vec::new();
    if !columns.is_empty() {
        let header = many(columns.iter().copied(), ctx);
        let row = row_of(header, content_text(columns.iter().copied()));
        groups.push(group(vec![row]));
    }
    if !body.is_empty() {
        groups.push(group(body));
    }
    groups
}

fn group(children: Vec<AriaNode>) -> AriaNode {
    AriaNode {
        role: "rowgroup".into(),
        children,
        ..AriaNode::default()
    }
}

fn row_of(children: Vec<AriaNode>, name: String) -> AriaNode {
    AriaNode {
        role: "row".into(),
        name,
        children,
        ..AriaNode::default()
    }
}

fn wrap_row<'a>(list: &[&InstChild<'a>], ctx: Ctx<'_, 'a>) -> AriaNode {
    row_of(vec![cell_of(list, ctx)], content_text(list.iter().copied()))
}

fn cell_of<'a>(list: &[&InstChild<'a>], ctx: Ctx<'_, 'a>) -> AriaNode {
    AriaNode {
        role: "cell".into(),
        name: content_text(list.iter().copied()),
        children: many(list.iter().copied(), ctx),
        ..AriaNode::default()
    }
}

fn row_cells<'a>(row: &Inst<'a>, ctx: Ctx<'_, 'a>) -> Vec<AriaNode> {
    let mut out = Vec::new();
    for c in &row.children {
        if is_kind(c, "cell") {
            out.extend(build(c, ctx));
        } else {
            out.push(cell_of(&[c], ctx));
        }
    }
    out
}

fn tabs_tree<'a>(n: &Inst<'a>, ctx: Ctx<'_, 'a>) -> Vec<AriaNode> {
    let tabs = n.tabs();
    let sel = n.selected_tab(&tabs);
    let mut list: Vec<AriaNode> = tabs
        .iter()
        .enumerate()
        .flat_map(|(i, t)| {
            let label = t.label();
            let extra = Extra::states(&[("selected", i == sel)]);
            node("tab", t, text_of(label), extra)
        })
        .collect();
    list.extend(many(n.children.iter().filter(|c| !is_kind(c, "tab")), ctx));
    let mut out = node("tablist", n, list, Extra::default());
    if let Some(panel) = tabs.get(sel) {
        let extra = Extra {
            name: Some(panel.label()),
            ..Extra::default()
        };
        out.extend(node("tabpanel", panel, many(panel.ordered(), ctx), extra));
    }
    out
}

/// The tree of an expanded root.
pub(super) fn of_root(root: &Inst<'_>) -> Vec<AriaNode> {
    build_node(root, Ctx { group: None })
}
