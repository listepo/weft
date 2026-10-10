//! Fragments (SPEC §10.7): one reading of `<fragment>` and `<param>` declarations, so that the
//! parser types their literals and the validator checks fragments and their reads alike.

use crate::diagnostics::{Code, one_of};
use crate::model::{
    ComponentDef, Content, Map, Node, PropDef, PropDefault, PropType, SlotDef, Value,
};
use crate::rules::is_name;

pub const FRAGMENT: &str = "fragment";
pub const PARAM: &str = "param";
pub const OUTLET: &str = "outlet";

pub enum ParamKind {
    Value(PropDef),
    Action,
    Slot(SlotDef),
}

/// A broken declaration: the `<param>` it is about (none for the root), the attribute (none for
/// the element), what is wrong and what would be right.
pub struct Problem {
    pub param: Option<usize>,
    pub attr: Option<String>,
    pub message: String,
    pub expected: String,
}

type Found = Vec<(Option<String>, String, String)>;

const TYPES: [&str; 7] = [
    "string", "number", "boolean", "enum", "token", "action", "slot",
];

fn value_type(t: &str) -> Option<PropType> {
    Some(match t {
        "string" => PropType::String,
        "number" => PropType::Number,
        "boolean" => PropType::Boolean,
        "enum" => PropType::Enum,
        "token" => PropType::Token,
        _ => return None,
    })
}

/// The literal type of a `<param>` attribute; `default` takes the parameter's own `type`.
pub fn param_attr_type(attr: &str, ty: Option<&str>) -> Option<PropType> {
    match attr {
        "min" | "max" => Some(PropType::Number),
        "integer" | "required" => Some(PropType::Boolean),
        "default" => value_type(ty?),
        _ => None,
    }
}

/// Both an attribute name of a `<use>` and a loop-variable name, so that a body can read it.
fn is_param_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s != FRAGMENT
        && s != "id"
}

fn allowed_attrs(ty: &str) -> &'static [&'static str] {
    match ty {
        "string" | "boolean" => &["name", "type", "required", "default"],
        "number" => &[
            "name", "type", "required", "default", "min", "max", "integer",
        ],
        "enum" => &["name", "type", "required", "default", "values"],
        "token" => &["name", "type", "required", "default", "token-type"],
        "slot" => &["name", "type", "required", "allowed-children", "content"],
        _ => &["name", "type"],
    }
}

fn words(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        Some(Value::String(s)) => Some(s.split_whitespace().map(str::to_owned).collect()),
        _ => None,
    }
}

/// Whether an attribute value has the form its declaration grammar needs.
fn well_formed(attr: &str, value: &Value) -> bool {
    match attr {
        "min" | "max" => matches!(value, Value::Number(n) if n.is_finite()),
        "integer" | "required" => matches!(value, Value::Bool(_)),
        "default" => true,
        "content" => *value == Value::String("nodes".into()),
        "allowed-children" => words(Some(value)).is_some_and(|w| w.iter().all(|k| is_name(k))),
        "values" => words(Some(value)).is_some_and(|w| !w.is_empty()),
        _ => matches!(value, Value::String(_)),
    }
}

/// Reads one `<param>`; the parameter is absent when its name or type is unusable.
fn read_param(node: &Node, found: &mut Found) -> Option<(String, ParamKind)> {
    let mut problem = |attr: Option<&str>, message: String, expected: &str| {
        found.push((attr.map(str::to_owned), message, expected.to_owned()));
    };
    let props = &node.props;
    let name = match props.get("name") {
        Some(Value::String(n)) if is_param_name(n) => Some(n.clone()),
        _ => {
            let message = "A parameter needs a name of lowercase letters and digits.";
            problem(
                Some("name"),
                message.into(),
                "[a-z][a-z0-9]*, not fragment or id",
            );
            None
        }
    };
    let ty = match props.get("type") {
        Some(Value::String(t)) if TYPES.contains(&t.as_str()) => t.as_str(),
        _ => {
            problem(
                Some("type"),
                "A parameter needs a known type.".into(),
                &one_of(TYPES),
            );
            return None;
        }
    };
    let content = !node.on.is_empty() || !node.slots.is_empty() || !node.children.is_empty();
    if node.id.is_some() || content {
        let expected = "<param name=\"…\" type=\"…\"/>";
        problem(
            None,
            "<param> takes no id, events or content.".into(),
            expected,
        );
    }
    let allowed = allowed_attrs(ty);
    for (attr, value) in props.iter().filter(|(a, _)| !a.starts_with("x-")) {
        if !allowed.contains(&attr.as_str()) {
            let message = format!("A {ty} parameter takes no \"{attr}\".");
            problem(Some(attr), message, &one_of(allowed));
        } else if !well_formed(attr, value) {
            let message = format!("\"{attr}\" of a parameter has the wrong form.");
            problem(
                Some(attr),
                message,
                "a literal of the grammar of SPEC §10.7",
            );
        }
    }
    let flag = |attr: &str| (props.get(attr) == Some(&Value::Bool(true))).then_some(true);
    let kind = match (ty, value_type(ty)) {
        (_, Some(kind)) => {
            let mut def = PropDef::new("", kind);
            def.values = words(props.get("values"));
            def.token_type = words(props.get("token-type")).map(|w| w.join(" "));
            def.min = props.get("min").and_then(number);
            def.max = props.get("max").and_then(number);
            def.integer = flag("integer");
            def.required = flag("required");
            if kind == PropType::Enum && def.values.is_none() {
                let message = "An enum parameter needs values.".into();
                problem(Some("values"), message, "values=\"a b\"");
            }
            if let Some(default) = props.get("default") {
                if def.required.is_some() {
                    let message = "A required parameter has no default.".into();
                    problem(Some("default"), message, "required or default, not both");
                }
                match default_of(default, &def) {
                    Some(d) => def.default = d,
                    None => {
                        let message = format!("The default is not a {ty} this parameter allows.");
                        problem(Some("default"), message, ty);
                    }
                }
            }
            ParamKind::Value(def)
        }
        ("action", _) => ParamKind::Action,
        _ => ParamKind::Slot(SlotDef {
            description: String::new(),
            allowed_children: words(props.get("allowed-children")),
            required: flag("required"),
        }),
    };
    Some((name?, kind))
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => Some(*n),
        _ => None,
    }
}

/// `Some(None)` is a valid default that a catalog default cannot hold: a token reference.
fn default_of(value: &Value, def: &PropDef) -> Option<Option<PropDefault>> {
    let in_range = |n: f64| {
        def.min.is_none_or(|m| n >= m)
            && def.max.is_none_or(|m| n <= m)
            && (def.integer.is_none() || n.fract() == 0.0)
    };
    match (def.kind, value) {
        (PropType::String, Value::String(s)) => Some(Some(PropDefault::String(s.clone()))),
        (PropType::Enum, Value::String(s)) if def.values.iter().flatten().any(|v| v == s) => {
            Some(Some(PropDefault::String(s.clone())))
        }
        (PropType::Number, Value::Number(n)) if in_range(*n) => Some(Some(PropDefault::Number(*n))),
        (PropType::Boolean, Value::Bool(b)) => Some(Some(PropDefault::Bool(*b))),
        (PropType::Token, Value::Token(_)) => Some(None),
        _ => None,
    }
}

/// The usable parameters of a fragment root, the first of each name.
pub fn params(root: &Node) -> Map<ParamKind> {
    let mut out = Map::new();
    let nodes = root.children.iter().filter_map(|c| c.as_node());
    for node in nodes.take_while(|n| n.kind == PARAM) {
        if let Some((name, kind)) = read_param(node, &mut Vec::new()) {
            out.entry(name).or_insert(kind);
        }
    }
    out
}

/// Every problem of a fragment root and its declarations (`W803`).
pub fn problems(root: &Node) -> Vec<Problem> {
    let mut found: Found = Vec::new();
    let mut out = Vec::new();
    if root.id.is_some() || !root.on.is_empty() || !root.slots.is_empty() {
        let expected = "<fragment label=\"…\">".into();
        found.push((
            None,
            "<fragment> takes no id, events or slots.".into(),
            expected,
        ));
    }
    for (name, value) in &root.props {
        // `version` is stored for T100, which checks the semver (`W230`). Rejecting it here
        // would make an inline fragment's version unwritable.
        let version = name == "version";
        let label = name == "label" && matches!(value, Value::String(_));
        if !name.starts_with("x-") && !version && !label {
            let message = format!("<fragment> takes no \"{name}\".");
            found.push((Some(name.clone()), message, "label=\"…\" as text".into()));
        }
    }
    let mut body = false;
    let mut names = Vec::new();
    for (index, child) in root.children.iter().enumerate() {
        let Some(node) = child.as_node() else {
            continue;
        };
        if node.kind != PARAM {
            body = true;
            continue;
        }
        let mut here: Found = Vec::new();
        if body {
            let expected = "every <param> before the body".into();
            here.push((None, "<param> comes after the body.".into(), expected));
        }
        if let Some((name, _)) = read_param(node, &mut here) {
            if names.contains(&name) {
                let message = format!("Parameter \"{name}\" is declared twice.");
                here.push((Some("name".into()), message, "a name declared once".into()));
            }
            names.push(name);
        }
        out.extend(here.into_iter().map(|(attr, message, expected)| Problem {
            param: Some(index),
            attr,
            message,
            expected,
        }));
    }
    if !body {
        let expected = "one or more elements after the parameters".into();
        found.push((None, "The fragment has no body.".into(), expected));
    }
    let root_problems = found.into_iter().map(|(attr, message, expected)| Problem {
        param: None,
        attr,
        message,
        expected,
    });
    root_problems.chain(out).collect()
}

/// Why a parameter so declared cannot be read where `def` is expected, if it cannot.
pub fn mismatch(name: &str, param: &PropDef, def: &PropDef) -> Option<(Code, String, String)> {
    let (want, have) = (def.kind.as_str(), param.kind.as_str());
    let widens = def.kind == PropType::String && param.kind == PropType::Enum;
    if def.kind != param.kind && !widens {
        let message = format!("Parameter \"{name}\" is a {have}; this attribute takes a {want}.");
        return Some((Code::W204, message, want.into()));
    }
    let allowed = def.values.clone().unwrap_or_default();
    let values = param.values.iter().flatten();
    if def.kind == PropType::Enum && values.into_iter().any(|v| !allowed.contains(v)) {
        let message = format!("Parameter \"{name}\" allows values this attribute does not.");
        return Some((Code::W203, message, one_of(&allowed)));
    }
    let wanted = def
        .token_type
        .as_ref()
        .filter(|_| def.kind == PropType::Token)?;
    let message = format!("Parameter \"{name}\" is not a token of type {wanted}.");
    (param.token_type.as_ref() != Some(wanted)).then(|| (Code::W307, message, wanted.clone()))
}

pub const USE: &str = "use";

/// The definition a `<use>` of this fragment answers to: its value parameters as props, its
/// action parameters as events and its slot parameters as slots.
pub fn signature(root: &Node) -> ComponentDef {
    let mut fragment = PropDef::new("The name of the fragment to place.", PropType::String);
    fragment.required = Some(true);
    fragment.bindable = Some(false);
    let mut props = Map::from_iter([(FRAGMENT.to_owned(), fragment)]);
    let (mut slots, mut events) = (Map::new(), Vec::new());
    for (name, kind) in params(root) {
        match kind {
            ParamKind::Value(def) => {
                props.insert(name, def);
            }
            ParamKind::Action => events.push(name),
            ParamKind::Slot(def) => {
                slots.insert(name, def);
            }
        }
    }
    let description = match root.props.get("label") {
        Some(Value::String(label)) => label.clone(),
        _ => String::new(),
    };
    ComponentDef {
        description,
        role: "group".to_owned(),
        content: Content::None,
        allowed_children: None,
        allowed_parents: None,
        requires_label: None,
        root: None,
        props: Some(props),
        slots: (!slots.is_empty()).then_some(slots),
        states: None,
        events: (!events.is_empty()).then_some(events),
    }
}
