//! Makes an imported document valid in lenient mode, as SPEC §9 promises: content a kind cannot
//! hold is dropped, required values get stand-ins, and anything the validator still rejects is
//! removed. Each change is a loss.

use weft_core::{
    Catalog, Child, Content, Mode, Node, Severity, ValidateOptions, Value, validate_document,
};

use super::read::element_path;
use super::{ImportResult, Loss, LossKind};

/// Rounds of "validate, remove what is rejected"; each round removes at least one thing.
const MAX_ROUNDS: usize = 64;

pub fn repair(result: &mut ImportResult, catalog: &Catalog) {
    let root = &mut result.document.root;
    fix(root, None, catalog, "", &mut result.losses, 0);
    for _ in 0..MAX_ROUNDS {
        let diagnostics = validate_document(
            &result.document,
            &ValidateOptions {
                catalog: Some(catalog),
                mode: Mode::Lenient,
                tokens: None,
                actions: None,
            },
        );
        let errors: Vec<_> = diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        if errors.is_empty() {
            return;
        }
        let mut changed = false;
        for e in &errors {
            changed |= remove_at(
                &mut result.document.root,
                &e.path,
                &e.message,
                &mut result.losses,
            );
        }
        if !changed {
            result.diagnostics.extend(errors);
            return;
        }
    }
}

fn lose(losses: &mut Vec<Loss>, kind: LossKind, path: &str, note: impl Into<String>) {
    losses.push(Loss {
        kind,
        path: path.to_owned(),
        note: note.into(),
    });
}

/// `parent` is the kind whose rules apply to `node`'s children: an `<each>` is transparent.
fn fix(
    node: &mut Node,
    parent: Option<&str>,
    catalog: &Catalog,
    path: &str,
    losses: &mut Vec<Loss>,
    depth: usize,
) {
    let here = element_path(path, node);
    let rules_kind = if node.kind == "each" {
        parent.unwrap_or("screen").to_owned()
    } else {
        node.kind.clone()
    };
    let def = catalog.components.get(&rules_kind);
    let takes_text = def.is_some_and(|d| matches!(d.content, Content::Text | Content::Mixed))
        && node.kind != "each";
    let takes_nodes = def.is_none_or(|d| matches!(d.content, Content::Nodes | Content::Mixed));
    let allowed = def.and_then(|d| d.allowed_children.clone());
    let mut kept = vec![];
    for child in std::mem::take(&mut node.children) {
        match child {
            Child::Text(t) if takes_text => kept.push(Child::Text(t)),
            Child::Text(_) => lose(
                losses,
                LossKind::Text,
                &here,
                format!("`{rules_kind}` holds no text; a text run is dropped"),
            ),
            Child::Node(mut n) => {
                let fits = takes_nodes
                    && allowed.as_ref().is_none_or(|a| a.contains(&n.kind))
                    && (n.kind == "each"
                        || catalog
                            .components
                            .get(&n.kind)
                            .and_then(|c| c.allowed_parents.as_ref())
                            .is_none_or(|p| p.contains(&rules_kind)));
                if fits && depth < super::MAX_DEPTH {
                    fix(&mut n, Some(&rules_kind), catalog, &here, losses, depth + 1);
                    if n.kind == "each" && n.children.is_empty() {
                        continue;
                    }
                    kept.push(Child::Node(n));
                } else {
                    lose(
                        losses,
                        LossKind::Structure,
                        &element_path(&here, &n),
                        format!(
                            "`{}` cannot be a child of `{rules_kind}` and is dropped",
                            n.kind
                        ),
                    );
                }
            }
        }
    }
    node.children = kept;
    for (name, list) in node.slots.iter_mut() {
        let slot_path = format!("{here}/slot[{name}]");
        let mut kept = vec![];
        for child in std::mem::take(list) {
            if let Child::Node(mut n) = child {
                fix(&mut n, None, catalog, &slot_path, losses, depth + 1);
                kept.push(Child::Node(n));
            }
        }
        *list = kept;
    }
    node.slots.retain(|_, l| !l.is_empty());
    if node.kind == "each" {
        return;
    }
    let Some(def) = def else { return };
    if def.requires_label == Some(true) && !node.props.contains_key("label") {
        lose(
            losses,
            LossKind::Names,
            &here,
            format!("`{}` needs a label; \"\" stands in", node.kind),
        );
        node.props
            .insert("label".to_owned(), Value::String(String::new()));
    }
    for (prop, pdef) in def.props.iter().flatten() {
        if pdef.required != Some(true) || prop == "weft" || node.props.contains_key(prop) {
            continue;
        }
        let stand_in = match prop.as_str() {
            "columns" => Value::Number(1.0),
            "level" => Value::Number(2.0),
            "value" => Value::String(text_of(node)),
            _ => Value::String(String::new()),
        };
        lose(
            losses,
            LossKind::Values,
            &here,
            format!("the required `{prop}` is missing; a stand-in is set"),
        );
        node.props.insert(prop.clone(), stand_in);
    }
}

fn text_of(node: &Node) -> String {
    node.children
        .iter()
        .filter_map(|c| match c {
            Child::Text(t) => Some(t.as_str()),
            Child::Node(_) => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Removes the attribute or element a diagnostic path names; false when it names nothing.
fn remove_at(root: &mut Node, path: &str, message: &str, losses: &mut Vec<Loss>) -> bool {
    let (element, attr) = match path.rsplit_once("/@") {
        Some((e, a)) => (e, Some(a)),
        None => (path, None),
    };
    let Some(id) = element
        .rsplit('/')
        .find_map(|seg| seg.split_once('#').map(|(_, id)| id))
    else {
        return false;
    };
    if let Some(attr) = attr {
        let Some(node) = find_mut(root, id, 0) else {
            return false;
        };
        let removed = match attr.strip_prefix("on-") {
            Some(event) => node.on.shift_remove(event).is_some(),
            None => node.props.shift_remove(attr).is_some(),
        };
        if removed {
            lose(
                losses,
                LossKind::Props,
                element,
                format!("`{attr}` is dropped: {message}"),
            );
        }
        return removed;
    }
    if root.id.as_deref() == Some(id) {
        return false;
    }
    let removed = remove_child(root, id, 0);
    if removed {
        lose(
            losses,
            LossKind::Structure,
            element,
            format!("the element is dropped: {message}"),
        );
    }
    removed
}

fn lists(node: &mut Node) -> impl Iterator<Item = &mut Vec<Child>> {
    std::iter::once(&mut node.children).chain(node.slots.values_mut())
}

fn find_mut<'n>(node: &'n mut Node, id: &str, depth: usize) -> Option<&'n mut Node> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    if depth > super::MAX_DEPTH + 1 {
        return None;
    }
    for list in lists(node) {
        for child in list.iter_mut() {
            if let Child::Node(n) = child
                && let Some(found) = find_mut(n, id, depth + 1)
            {
                return Some(found);
            }
        }
    }
    None
}

fn remove_child(node: &mut Node, id: &str, depth: usize) -> bool {
    if depth > super::MAX_DEPTH + 1 {
        return false;
    }
    for list in lists(node) {
        if let Some(i) = list
            .iter()
            .position(|c| matches!(c, Child::Node(n) if n.id.as_deref() == Some(id)))
        {
            list.remove(i);
            return true;
        }
        for child in list.iter_mut() {
            if let Child::Node(n) = child
                && remove_child(n, id, depth + 1)
            {
                return true;
            }
        }
    }
    false
}
