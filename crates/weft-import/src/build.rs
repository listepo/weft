//! Assembles a Weft document from the role tree so that it validates against the catalog in
//! lenient mode: whatever would break a catalog rule is placed in a slot that admits it or kept as
//! an `x-aria-<role>` extension element (SPEC §8), and every such step is a loss.

use std::collections::HashSet;

use weft_core::{
    ARIA_ROLES, Catalog, Child, ComponentDef, Content, Diagnostic, Document, Map, Node,
    ValidateOptions, Value, WEFT_VERSION, canonicalize, is_id, validate_document,
};

use crate::ids::IdState;
use crate::kinds::{KindIndex, dissolved};
use crate::limits::{MAX_DEPTH, MAX_NODES, limit_reached};
use crate::loss::{ImportResult, LossKind, Losses};
use crate::props::{Scalar, coerce, fill_required};
use crate::sem::Sem;
use crate::text::{clean, js_trim, literal, squash};

const BUSY_STATES: &[&str] = &["busy", "loading", "submitting"];

pub struct BuildOptions<'a> {
    pub catalog: &'a Catalog,
    /// Ids the source carries; generated ids avoid them.
    pub reserved: Vec<String>,
    /// Diagnostics found before the build (limits, unreadable parts); validation adds to them.
    pub diagnostics: Vec<Diagnostic>,
}

pub struct Built {
    pub result: ImportResult,
    /// The path of the root element, where whole-input losses are reported.
    pub root_path: String,
}

/// The text a subtree shows, joined with spaces as an accessible name computed from content is.
pub fn text_of<'s>(list: impl IntoIterator<Item = &'s Sem>, depth: usize) -> String {
    if depth > MAX_DEPTH {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    for s in list {
        let inner = if s.role == "text" {
            s.name.clone()
        } else {
            let t = text_of(&s.children, depth + 1);
            if t.is_empty() { s.name.clone() } else { t }
        };
        if !js_trim(&inner).is_empty() {
            parts.push(inner);
        }
    }
    squash(&parts.join(" "))
}

#[derive(Clone)]
struct Parent<'c> {
    kind: String,
    def: Option<&'c ComponentDef>,
    path: String,
    in_form: bool,
    depth: usize,
}

struct Ctx<'c> {
    index: KindIndex<'c>,
    form_kind: Option<&'c str>,
    ids: IdState,
    claimed: HashSet<String>,
    losses: Losses,
    diagnostics: Vec<Diagnostic>,
    nodes: usize,
    truncated: bool,
}

struct Placed {
    node: Node,
    slot: Option<String>,
}

impl<'c> Ctx<'c> {
    fn lose(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    fn truncate(&mut self, path: &str) {
        if self.truncated {
            return;
        }
        self.truncated = true;
        limit_reached(
            &mut self.diagnostics,
            path,
            "is larger or deeper than the import limit",
        );
    }

    fn literal(&mut self, path: &str, s: &str) -> String {
        literal(&mut self.losses, path, s)
    }

    /// A generated id reads as the kind plus the element's name, or its text for kinds that show
    /// text; containers without a name are numbered.
    fn take_id(&mut self, s: &Sem, base: &str, shows_text: bool) -> String {
        if let Some(id) = &s.id
            && is_id(id)
            && !self.claimed.contains(id)
        {
            self.claimed.insert(id.clone());
            self.ids.used.insert(id.clone());
            return id.clone();
        }
        let name = if !s.name.is_empty() {
            s.name.clone()
        } else if shows_text {
            text_of(&s.children, 0)
        } else {
            String::new()
        };
        self.ids.fresh(base, &name)
    }

    fn report(&mut self, s: &Sem, path: &str) {
        for n in &s.notes {
            self.losses.push(n.kind, path, n.note.clone());
        }
    }

    fn flatten<'s>(&mut self, list: &'s [Sem], depth: usize, path: &str) -> Vec<&'s Sem> {
        let mut out = Vec::new();
        self.flatten_into(list, depth, path, &mut out);
        out
    }

    fn flatten_into<'s>(
        &mut self,
        list: &'s [Sem],
        depth: usize,
        path: &str,
        out: &mut Vec<&'s Sem>,
    ) {
        for s in list {
            if s.kind.is_some() || !dissolved(&s.role) {
                out.push(s);
                continue;
            }
            if depth > MAX_DEPTH {
                self.truncate(path);
                continue;
            }
            self.flatten_into(&s.children, depth + 1, path, out);
        }
    }
}

fn placement(
    kind: &str,
    def: &ComponentDef,
    parent: &Parent<'_>,
) -> Result<Option<String>, String> {
    if kind == "screen" {
        return Err("a screen appears only at the root".into());
    }
    // Extension parents are opaque: parent/child rules skip them (SPEC §8).
    let Some(pdef) = parent.def else {
        return Ok(None);
    };
    if let Some(allowed) = &def.allowed_parents
        && !allowed.contains(&parent.kind)
    {
        return Err(format!("<{kind}> belongs only in {}", allowed.join(", ")));
    }
    let Some(allowed) = &pdef.allowed_children else {
        return Ok(None);
    };
    if allowed.iter().any(|k| k == kind) {
        return Ok(None);
    }
    let mut slots: Vec<_> = pdef.slots.iter().flatten().collect();
    slots.sort_by(|a, b| a.0.cmp(b.0));
    for (name, slot) in slots {
        if slot
            .allowed_children
            .as_ref()
            .is_none_or(|a| a.iter().any(|k| k == kind))
        {
            return Ok(Some(name.clone()));
        }
    }
    Err(format!("<{}> does not hold <{kind}>", parent.kind))
}

fn set_prop(
    ctx: &mut Ctx<'_>,
    props: &mut Map<Value>,
    def: &ComponentDef,
    name: &str,
    raw: &Scalar,
    path: &str,
    in_form: bool,
) {
    if name == "state" {
        match raw {
            Scalar::String(s) if def.states.as_ref().is_some_and(|st| st.contains(s)) => {
                props.insert("state".into(), Value::String(s.clone()));
            }
            _ => ctx.lose(
                LossKind::Props,
                path,
                format!("state \"{}\" is not a state of this component", raw.text()),
            ),
        }
        return;
    }
    let Some(pd) = def.prop(name) else {
        return;
    };
    if name == "text" {
        return;
    }
    // SPEC §5.1: a submit button outside a form is an error (W313), so the flag is not kept there.
    if name == "submit" && !in_form {
        ctx.lose(
            LossKind::Props,
            path,
            "a submit button outside a form is imported as a plain button",
        );
        return;
    }
    match coerce(pd, raw) {
        None => ctx.lose(
            LossKind::Props,
            path,
            format!("{name}=\"{}\" is not a valid value", raw.text()),
        ),
        Some(Value::String(s)) => {
            let s = ctx.literal(path, &s);
            props.insert(name.into(), Value::String(s));
        }
        Some(value) => {
            props.insert(name.into(), value);
        }
    }
}

struct Converted {
    children: Vec<Child>,
    slots: Map<Vec<Child>>,
    chosen: Option<Node>,
}

// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn convert_list<'c>(list: &[Sem], parent: &Parent<'c>, ctx: &mut Ctx<'c>) -> Converted {
    let mut items = ctx.flatten(list, parent.depth, &parent.path);
    let mut out = Converted {
        children: Vec::new(),
        slots: Map::new(),
        chosen: None,
    };
    let mut pending = String::new();
    let mut i = 0;
    while i < items.len() {
        let s = items[i];
        if s.role == "text" {
            pending.push(' ');
            pending.push_str(&s.name);
            i += 1;
            continue;
        }
        flush(&mut pending, parent, ctx, &mut out);
        if s.kind.is_none() && s.role == "tablist" {
            let mut panels = Vec::new();
            while let Some(next) = items.get(i + 1)
                && next.role == "tabpanel"
                && next.kind.is_none()
            {
                panels.push(*next);
                i += 1;
            }
            for p in convert_tabs(s, &panels, parent, ctx) {
                place(&mut out, p);
            }
            i += 1;
            continue;
        }
        if is_header_row(s, parent, ctx) {
            let cells = ctx.flatten(&s.children, parent.depth, &parent.path);
            items.splice(i + 1..i + 1, cells);
            i += 1;
            continue;
        }
        let Some(placed) = convert_node(s, parent, ctx) else {
            i += 1;
            continue;
        };
        for state in ["checked", "selected"] {
            // A state the child kind cannot hold is the parent's choice (radio in radio-group,
            // option in select); `node.props.value` names the choice.
            let own = placed.node.props.contains_key(state);
            if s.states.get(state) == Some(&Scalar::Bool(true))
                && !own
                && placed.node.props.contains_key("value")
                && out.chosen.is_none()
            {
                out.chosen = Some(placed.node.clone());
            }
        }
        place(&mut out, placed);
        i += 1;
    }
    flush(&mut pending, parent, ctx, &mut out);
    out
}

fn place(out: &mut Converted, p: Placed) {
    match p.slot {
        None => out.children.push(Child::Node(Box::new(p.node))),
        Some(slot) => out
            .slots
            .entry(slot)
            .or_default()
            .push(Child::Node(Box::new(p.node))),
    }
}

// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn flush<'c>(pending: &mut String, parent: &Parent<'c>, ctx: &mut Ctx<'c>, out: &mut Converted) {
    let text = squash(&clean(pending));
    pending.clear();
    if text.is_empty() {
        return;
    }
    let content = parent.def.map_or(Content::Mixed, |d| d.content);
    match (content, ctx.index.text) {
        (Content::Mixed, _) => out.children.push(Child::Text(text)),
        (Content::Nodes, Some(text_kind)) => {
            let mut run = Sem::new("paragraph", text.clone());
            run.kind = Some(text_kind.to_owned());
            run.children = vec![Sem::text(text)];
            if let Some(placed) = convert_node(&run, parent, ctx) {
                place(out, placed);
            }
        }
        _ => ctx.lose(
            LossKind::Text,
            &parent.path,
            format!("text \"{text}\" has no place in <{}>", parent.kind),
        ),
    }
}

/// The header row a renderer emits for a table's columns (SPEC §5.1 notes) dissolves back into the
/// table's column children.
fn is_header_row(s: &Sem, parent: &Parent<'_>, ctx: &mut Ctx<'_>) -> bool {
    if s.kind.is_some() || s.role != "row" {
        return false;
    }
    if ctx.index.kind_of("table") != Some(parent.kind.as_str()) {
        return false;
    }
    let cells = ctx.flatten(&s.children, parent.depth, &parent.path);
    cells.iter().any(|c| c.role == "columnheader")
        && cells
            .iter()
            .all(|c| c.role == "columnheader" || (c.role == "text" && js_trim(&c.name).is_empty()))
}

// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn convert_node<'c>(s: &Sem, parent: &Parent<'c>, ctx: &mut Ctx<'c>) -> Option<Placed> {
    ctx.nodes += 1;
    if ctx.nodes > MAX_NODES || parent.depth >= MAX_DEPTH {
        ctx.truncate(&parent.path);
        return None;
    }
    let Some(target) = ctx.index.resolve(&s.role, s.kind.as_deref()) else {
        return Some(Placed {
            node: extension(s, parent, ctx, None),
            slot: None,
        });
    };
    match placement(&target.kind, target.def, parent) {
        Err(why) => Some(Placed {
            node: extension(s, parent, ctx, Some(why)),
            slot: None,
        }),
        Ok(slot) => Some(Placed {
            node: component_node(
                s,
                &target.kind,
                target.def,
                target.preset,
                parent,
                slot.as_deref(),
                ctx,
            ),
            slot,
        }),
    }
}

// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn extension<'c>(s: &Sem, parent: &Parent<'c>, ctx: &mut Ctx<'c>, why: Option<String>) -> Node {
    let role = if ARIA_ROLES.contains(&s.role.as_str()) && !dissolved(&s.role) {
        s.role.as_str()
    } else {
        "group"
    };
    let kind = format!("x-aria-{role}");
    let id = ctx.take_id(s, role, false);
    let path = format!("{}/{kind}#{id}", parent.path);
    ctx.report(s, &path);
    let note = why.unwrap_or_else(|| {
        if role == s.role {
            format!("role {role} has no kind in the catalog")
        } else {
            format!("{} is not a WAI-ARIA role", quote(&s.role))
        }
    });
    ctx.lose(LossKind::Kinds, &path, note);
    let mut props = Map::new();
    props.insert("role".into(), Value::String(role.into()));
    let name = squash(&s.name);
    if !name.is_empty() {
        let label = ctx.literal(&path, &name);
        props.insert("label".into(), Value::String(label));
    }
    let inner = convert_list(
        &s.children,
        &Parent {
            kind: kind.clone(),
            def: None,
            path,
            in_form: parent.in_form,
            depth: parent.depth + 1,
        },
        ctx,
    );
    let mut node = Node::new(kind);
    node.id = Some(id);
    node.props = props;
    node.children = inner.children;
    node
}

/// `"…"` as a template literal writes it: the text between plain double quotes, unescaped.
fn quote(s: &str) -> String {
    format!("\"{s}\"")
}

// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn component_node<'c>(
    s: &Sem,
    kind: &str,
    def: &'c ComponentDef,
    preset: &[(&str, &str)],
    parent: &Parent<'c>,
    slot: Option<&str>,
    ctx: &mut Ctx<'c>,
) -> Node {
    let id = ctx.take_id(s, kind, def.content == Content::Text);
    let path = format!(
        "{}{}/{kind}#{id}",
        parent.path,
        slot.map_or(String::new(), |s| format!("/slot[{s}]"))
    );
    ctx.report(s, &path);
    let mut props: Map<Value> = Map::new();
    let mut given: Map<Scalar> = Map::new();
    for (name, raw) in preset {
        given.insert((*name).to_owned(), Scalar::from(*raw));
    }
    for (name, raw) in &s.props {
        given.insert(name.clone(), Scalar::String(raw.clone()));
    }
    for (name, raw) in &given {
        set_prop(ctx, &mut props, def, name, raw, &path, parent.in_form);
    }
    for (name, raw) in &s.states {
        let boolean = def
            .prop(name)
            .is_some_and(|p| p.kind == weft_core::PropType::Boolean);
        if name == "level" || boolean {
            if *raw != Scalar::Bool(false) && def.prop(name).is_some() {
                set_prop(ctx, &mut props, def, name, raw, &path, parent.in_form);
            }
        } else if name == "invalid" && *raw == Scalar::Bool(true) && !props.contains_key("state") {
            if def.states.iter().flatten().any(|s| s == "invalid") {
                props.insert("state".into(), Value::String("invalid".into()));
            }
        } else if name == "busy" && *raw == Scalar::Bool(true) && !props.contains_key("state") {
            let states = def.states.as_deref().unwrap_or_default();
            if let Some(busy) = BUSY_STATES.iter().find(|b| states.iter().any(|s| s == *b)) {
                props.insert("state".into(), Value::String((*busy).into()));
            }
        }
    }

    let name = squash(&s.name);
    let mut children: Vec<Child> = Vec::new();
    let mut slots: Map<Vec<Child>> = Map::new();
    let mut content = String::new();
    match def.content {
        Content::None => {
            // An input's value is the text it exposes (a textbox's text run in a snapshot).
            let shown = text_of(&s.children, 0);
            if !shown.is_empty() && def.prop("value").is_some() && !props.contains_key("value") {
                set_prop(
                    ctx,
                    &mut props,
                    def,
                    "value",
                    &Scalar::String(shown),
                    &path,
                    parent.in_form,
                );
            } else if !shown.is_empty() {
                ctx.lose(
                    LossKind::Text,
                    &path,
                    format!("content \"{shown}\" has no place in <{kind}>"),
                );
            }
        }
        Content::Text => {
            content = text_of(&s.children, 0);
            if content.is_empty() {
                content = name.clone();
            }
            if !content.is_empty() {
                children = vec![Child::Text(clean(&content))];
            }
        }
        Content::Nodes | Content::Mixed => {
            let inner = convert_list(
                &s.children,
                &Parent {
                    kind: kind.to_owned(),
                    def: Some(def),
                    path: path.clone(),
                    in_form: parent.in_form || Some(kind) == ctx.form_kind,
                    depth: parent.depth + 1,
                },
                ctx,
            );
            children = inner.children;
            slots = inner.slots;
            for (slot_name, list) in &slots {
                if !list.is_empty() {
                    ctx.lose(
                        LossKind::Slots,
                        &format!("{path}/slot[{slot_name}]"),
                        format!(
                            "content <{kind}> does not hold by default is placed in its {slot_name} slot"
                        ),
                    );
                }
            }
            content = text_of(&s.children, 0);
            let value = inner.chosen.and_then(|c| c.props.get("value").cloned());
            if let Some(value) = value
                && def.prop("value").is_some_and(|p| p.writable == Some(true))
                && !props.contains_key("value")
            {
                props.insert("value".into(), value);
            }
        }
    }
    // A name that only repeats the content is computed from it, not a label of its own.
    let requires_label = def.requires_label == Some(true);
    if !name.is_empty() && (name != content || requires_label) {
        let label = ctx.literal(&path, &name);
        props.insert("label".into(), Value::String(label));
    }
    if requires_label && !props.contains_key("label") {
        props.insert("label".into(), Value::String(String::new()));
        ctx.lose(
            LossKind::Names,
            &path,
            format!("<{kind}> needs an accessible name and the input gives none"),
        );
    }
    let text = {
        let t = text_of(&s.children, 0);
        if t.is_empty() { s.name.clone() } else { t }
    };
    fill_required(
        &mut ctx.losses,
        &mut props,
        def,
        parent.def,
        parent.kind.is_empty(),
        &text,
        &path,
    );
    let mut node = Node::new(kind);
    node.id = Some(id);
    node.props = props;
    node.slots = slots;
    node.children = children;
    node
}

/// SPEC §5.1: a renderer emits one tablist of tab buttons followed by tab panels; each panel goes
/// back into the tab it belongs to.
// Kept out of line: inlined into each other, these share one large frame per nesting level, and
// imports must fit the 1 MiB stack WebAssembly gets.
#[inline(never)]
fn convert_tabs<'c>(
    list: &Sem,
    panels: &[&Sem],
    parent: &Parent<'c>,
    ctx: &mut Ctx<'c>,
) -> Vec<Placed> {
    let tabs_kind = ctx.index.kind_of("tablist");
    let tab_kind = ctx.index.kind_of("tab");
    let tabs_def = tabs_kind.and_then(|k| ctx.index.component(k));
    let tab_def = tab_kind.and_then(|k| ctx.index.component(k));
    let fallback = |ctx: &mut Ctx<'c>| {
        std::iter::once(list)
            .chain(panels.iter().copied())
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|s| convert_node(s, parent, ctx))
            .collect::<Vec<_>>()
    };
    let (Some(tabs_kind), Some(tabs_def), Some(tab_kind), Some(tab_def)) =
        (tabs_kind, tabs_def, tab_kind, tab_def)
    else {
        return fallback(ctx);
    };
    let Ok(slot) = placement(tabs_kind, tabs_def, parent) else {
        return fallback(ctx);
    };
    ctx.nodes += 1;
    if ctx.nodes > MAX_NODES || parent.depth >= MAX_DEPTH {
        ctx.truncate(&parent.path);
        return vec![];
    }
    let id = ctx.take_id(list, tabs_kind, false);
    let path = format!(
        "{}{}/{tabs_kind}#{id}",
        parent.path,
        slot.as_deref()
            .map_or(String::new(), |s| format!("/slot[{s}]"))
    );
    ctx.report(list, &path);
    let items = ctx.flatten(&list.children, parent.depth + 1, &path);
    let tabs: Vec<usize> = (0..items.len())
        .filter(|&i| items[i].kind.is_none() && items[i].role == "tab")
        .collect();
    // (panel, the item index of the tab that owns it), in the order panels were matched.
    let mut owner: Vec<(usize, usize)> = Vec::new();
    let mut extra: Vec<&Sem> = Vec::new();
    for (p, panel) in panels.iter().enumerate() {
        let free = |t: usize, owner: &[(usize, usize)]| !owner.iter().any(|&(_, o)| o == t);
        let labelled = panel.labelled_by.as_deref().unwrap_or_default();
        let found = tabs
            .iter()
            .copied()
            .find(|&t| {
                free(t, &owner)
                    && items[t]
                        .reference
                        .as_ref()
                        .is_some_and(|r| labelled.contains(r))
            })
            .or_else(|| {
                tabs.iter().copied().find(|&t| {
                    let shown = if items[t].name.is_empty() {
                        text_of(&items[t].children, 0)
                    } else {
                        items[t].name.clone()
                    };
                    free(t, &owner)
                        && !panel.name.is_empty()
                        && squash(&shown) == squash(&panel.name)
                })
            })
            .or_else(|| {
                tabs.iter().copied().find(|&t| {
                    free(t, &owner) && items[t].states.get("selected") == Some(&Scalar::Bool(true))
                })
            })
            .or_else(|| tabs.iter().copied().find(|&t| free(t, &owner)));
        match found {
            Some(t) => owner.push((p, t)),
            None => extra.push(panel),
        }
    }
    let me = Parent {
        kind: tabs_kind.to_owned(),
        def: Some(tabs_def),
        path: path.clone(),
        in_form: parent.in_form,
        depth: parent.depth + 1,
    };
    let mut children: Vec<Child> = Vec::new();
    let mut selected: Option<String> = None;
    for (i, item) in items.iter().enumerate() {
        if item.role == "text" {
            continue;
        }
        if !tabs.contains(&i) {
            if let Some(placed) = convert_node(item, &me, ctx) {
                children.push(Child::Node(Box::new(placed.node)));
            }
            continue;
        }
        let tab_id = ctx.take_id(item, tab_kind, false);
        let tab_path = format!("{path}/{tab_kind}#{tab_id}");
        ctx.report(item, &tab_path);
        let panel = owner
            .iter()
            .find(|&&(_, t)| t == i)
            .map(|&(p, _)| panels[p]);
        let inner = panel.map(|panel| {
            convert_list(
                &panel.children,
                &Parent {
                    kind: tab_kind.to_owned(),
                    def: Some(tab_def),
                    path: tab_path.clone(),
                    in_form: parent.in_form,
                    depth: parent.depth + 2,
                },
                ctx,
            )
        });
        if panel.is_none() {
            ctx.lose(
                LossKind::Hidden,
                &tab_path,
                "the panel of this tab is not in the input (only the selected panel is exposed)",
            );
        }
        let label_text = {
            let n = squash(&item.name);
            if n.is_empty() {
                text_of(&item.children, 0)
            } else {
                n
            }
        };
        let label = ctx.literal(&tab_path, &label_text);
        let mut node = Node::new(tab_kind);
        node.id = Some(tab_id.clone());
        node.props.insert("label".into(), Value::String(label));
        if let Some(inner) = inner {
            node.children = inner.children;
            node.slots = inner.slots;
        }
        if item.states.get("selected") == Some(&Scalar::Bool(true)) && selected.is_none() {
            selected = Some(tab_id);
        }
        children.push(Child::Node(Box::new(node)));
    }
    let mut props = Map::new();
    let name = squash(&list.name);
    if !name.is_empty() {
        let label = ctx.literal(&path, &name);
        props.insert("label".into(), Value::String(label));
    }
    if let Some(selected) = selected
        && tabs_def
            .prop("selected")
            .is_some_and(|p| p.kind == weft_core::PropType::String)
    {
        props.insert("selected".into(), Value::String(selected));
    }
    let mut node = Node::new(tabs_kind);
    node.id = Some(id);
    node.props = props;
    node.children = children;
    let mut out = vec![Placed { node, slot }];
    for p in extra {
        if let Some(placed) = convert_node(p, parent, ctx) {
            out.push(placed);
        }
    }
    out
}

/// Builds and validates the document; `top` is the input's top level.
pub fn build_document(top: &[Sem], options: BuildOptions<'_>) -> Built {
    let index = KindIndex::new(options.catalog);
    let form_kind = index.kind_of("form");
    let mut ctx = Ctx {
        index,
        form_kind,
        ids: IdState {
            used: options.reserved.into_iter().collect(),
            ..IdState::default()
        },
        claimed: HashSet::new(),
        losses: Losses::default(),
        diagnostics: options.diagnostics,
        nodes: 0,
        truncated: false,
    };
    let items = ctx.flatten(top, 0, "");
    let shown: Vec<&Sem> = items
        .iter()
        .copied()
        .filter(|s| s.role != "text" || !js_trim(&s.name).is_empty())
        .collect();
    let main = match shown.as_slice() {
        [only]
            if only.kind.as_deref() == Some("screen")
                || (only.kind.is_none() && only.role == "main") =>
        {
            Some(*only)
        }
        _ => None,
    };
    let synthesized;
    let root: &Sem = match main {
        Some(m) => m,
        None => {
            let mut r = Sem::new("main", "");
            r.children = items.iter().map(|s| (*s).clone()).collect();
            if !ctx.ids.used.contains("screen") {
                r.id = Some("screen".into());
            }
            synthesized = r;
            &synthesized
        }
    };
    let none = Parent {
        kind: String::new(),
        def: None,
        path: String::new(),
        in_form: false,
        depth: 0,
    };
    let node = match ctx.index.component("screen") {
        Some(def) => component_node(root, "screen", def, &[], &none, None, &mut ctx),
        None => extension_root(root, &mut ctx),
    };
    let root_path = format!("/screen#{}", node.id.as_deref().unwrap_or_default());
    if main.is_none() {
        ctx.lose(
            LossKind::Structure,
            &root_path,
            "the input has no single main landmark; a screen was added as the root",
        );
    }
    let document = canonicalize(&Document {
        weft: WEFT_VERSION.into(),
        root: node,
    });
    let mut diagnostics = ctx.diagnostics;
    diagnostics.extend(validate_document(
        &document,
        &ValidateOptions {
            catalog: Some(options.catalog),
            ..ValidateOptions::default()
        },
    ));
    Built {
        result: ImportResult {
            document,
            losses: ctx.losses.0,
            diagnostics,
        },
        root_path,
    }
}

/// A catalog without `screen` still gets the root SPEC §2 requires; validation reports the rest.
fn extension_root(root: &Sem, ctx: &mut Ctx<'_>) -> Node {
    let id = ctx.take_id(root, "screen", false);
    let inner = convert_list(
        &root.children,
        &Parent {
            kind: "screen".into(),
            def: None,
            path: format!("/screen#{id}"),
            in_form: false,
            depth: 1,
        },
        ctx,
    );
    let mut node = Node::new("screen");
    node.id = Some(id);
    node.children = inner.children;
    node
}
