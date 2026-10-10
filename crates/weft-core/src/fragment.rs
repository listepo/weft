//! Fragments (SPEC §10.7): one reading of `<fragment>` and `<param>` declarations, so that the
//! parser types their literals and the validator checks fragments and their reads alike.

use crate::diagnostics::{Code, one_of};
use crate::model::{
    Child, ComponentDef, Content, Map, Node, PropDef, PropDefault, PropType, SlotDef, Value,
};
use crate::rules::is_name;

pub const FRAGMENT: &str = "fragment";
pub const PARAM: &str = "param";
pub const OUTLET: &str = "outlet";
pub const VARIANT: &str = "variant";

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
        "integer" | "required" | "variant" => Some(PropType::Boolean),
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
        // `variant` is not a prop of the parameter's type. W809 owns it.
        if attr == "variant" {
            continue;
        }
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

/// The one enum parameter marked `variant="true"`, when its name is usable.
struct Axis<'a> {
    index: usize,
    name: &'a str,
    values: Vec<String>,
    param: &'a Node,
}

fn each_param(root: &Node) -> impl Iterator<Item = (usize, &Node)> {
    root.children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| {
            let node = child.as_node()?;
            (node.kind == PARAM).then_some((index, node))
        })
}

fn axis(root: &Node) -> Option<Axis<'_>> {
    each_param(root).find_map(|(index, param)| {
        if param.props.get("variant") != Some(&Value::Bool(true)) {
            return None;
        }
        let enum_ = matches!(param.props.get("type"), Some(Value::String(t)) if t == "enum");
        let Value::String(name) = param.props.get("name")? else {
            return None;
        };
        (enum_ && is_param_name(name)).then(|| Axis {
            index,
            name,
            values: words(param.props.get("values")).unwrap_or_default(),
            param,
        })
    })
}

fn declaration_len(root: &Node) -> usize {
    root.children
        .iter()
        .take_while(|c| c.as_node().is_some_and(|n| n.kind == PARAM))
        .count()
}

fn problem(param: Option<usize>, attr: Option<&str>, message: &str, expected: &str) -> Problem {
    Problem {
        param,
        attr: attr.map(str::to_owned),
        message: message.to_owned(),
        expected: expected.to_owned(),
    }
}

/// Whether this fragment selects its body with a variant parameter.
pub fn has_variant_param(root: &Node) -> bool {
    axis(root).is_some()
}

/// `W809` for a variant parameter or `<variant>` that does not match §10.7.
pub fn variant_problems(root: &Node) -> Vec<Problem> {
    let mut out = Vec::new();
    let mut count = 0usize;
    for (index, param) in each_param(root) {
        if !param.props.contains_key("variant") {
            continue;
        }
        let marked = param.props.get("variant") == Some(&Value::Bool(true));
        let enum_ = matches!(param.props.get("type"), Some(Value::String(t)) if t == "enum");
        if !marked || !enum_ {
            out.push(problem(
                Some(index),
                Some("variant"),
                "variant=\"true\" belongs on one enum parameter.",
                "type=\"enum\" variant=\"true\"",
            ));
            continue;
        }
        let Some(Value::String(name)) = param.props.get("name") else {
            continue;
        };
        if !is_param_name(name) {
            continue;
        }
        count += 1;
        if count > 1 {
            out.push(problem(
                Some(index),
                Some("variant"),
                "A fragment has at most one variant parameter.",
                "one variant=\"true\"",
            ));
        }
    }
    let Some(axis) = axis(root) else {
        for (index, child) in root.children.iter().enumerate() {
            if child.as_node().is_some_and(|n| n.kind == VARIANT) {
                out.push(problem(
                    Some(index),
                    None,
                    "<variant> needs a variant parameter.",
                    "<param type=\"enum\" variant=\"true\">",
                ));
            }
        }
        return out;
    };
    let mut covered: Vec<String> = Vec::new();
    for (index, child) in root.children.iter().enumerate() {
        let Some(node) = child.as_node() else {
            out.push(problem(
                Some(index),
                None,
                "A fragment with a variant parameter contains only <variant> after <param>.",
                "<variant when=\"…\">",
            ));
            continue;
        };
        if node.kind == PARAM {
            continue;
        }
        if node.kind != VARIANT {
            out.push(problem(
                Some(index),
                None,
                "A fragment with a variant parameter contains only <variant> after <param>.",
                "<variant when=\"…\">",
            ));
            continue;
        }
        check_variant(node, index, &axis.values, &mut covered, &mut out);
    }
    for value in &axis.values {
        if !covered.iter().any(|c| c == value) {
            out.push(problem(
                Some(axis.index),
                Some("values"),
                &format!("Value \"{value}\" is covered by no <variant>."),
                &format!("when=\"{value}\""),
            ));
        }
    }
    out
}

fn check_variant(
    node: &Node,
    index: usize,
    values: &[String],
    covered: &mut Vec<String>,
    out: &mut Vec<Problem>,
) {
    if node.id.is_some() || !node.on.is_empty() || !node.slots.is_empty() {
        out.push(problem(
            Some(index),
            None,
            "<variant> takes only when.",
            "<variant when=\"…\">",
        ));
    }
    for attr in node.props.keys() {
        if attr != "when" {
            out.push(problem(
                Some(index),
                Some(attr),
                "<variant> takes only when.",
                "when=\"…\"",
            ));
        }
    }
    let Some(Value::String(when)) = node.props.get("when") else {
        out.push(problem(
            Some(index),
            Some("when"),
            "<variant> needs when listing values of the variant parameter.",
            "when=\"…\"",
        ));
        return;
    };
    let words: Vec<&str> = when.split_whitespace().collect();
    if words.is_empty() {
        out.push(problem(
            Some(index),
            Some("when"),
            "<variant> needs when listing values of the variant parameter.",
            "when=\"…\"",
        ));
    }
    for word in words {
        if !values.is_empty() && !values.iter().any(|v| v == word) {
            out.push(problem(
                Some(index),
                Some("when"),
                &format!("when lists \"{word}\", which the variant parameter does not have."),
                "a value of the variant parameter",
            ));
            continue;
        }
        if covered.iter().any(|c| c == word) {
            out.push(problem(
                Some(index),
                Some("when"),
                &format!("Value \"{word}\" is covered by two <variant> elements."),
                "each value in one when",
            ));
            continue;
        }
        covered.push(word.to_owned());
    }
    if !node.children.iter().any(|c| c.as_node().is_some()) {
        out.push(problem(
            Some(index),
            None,
            "<variant> holds one or more elements.",
            "one or more elements",
        ));
    }
}

/// How a `<use>` resolves the variant parameter. `None` when the fragment has no such parameter.
pub struct Choice<'a> {
    pub name: &'a str,
    /// The literal: the attribute, or the default when the attribute is absent.
    pub value: Option<&'a str>,
    pub defaulted: bool,
    /// The stored value the choice was read from, for readback.
    pub raw: Option<&'a Value>,
}

pub fn choice<'a>(root: &'a Node, use_node: &'a Node) -> Option<Choice<'a>> {
    let axis = axis(root)?;
    let (value, defaulted, raw) = match use_node.props.get(axis.name) {
        Some(raw @ Value::String(value)) => (Some(value.as_str()), false, Some(raw)),
        Some(raw) => (None, false, Some(raw)),
        None => match axis.param.props.get("default") {
            Some(raw @ Value::String(value)) => (Some(value.as_str()), true, Some(raw)),
            _ => (None, false, None),
        },
    };
    Some(Choice {
        name: axis.name,
        value,
        defaulted,
        raw,
    })
}

/// Children of the `<variant>` whose `when` lists `value`.
pub fn variant_body<'a>(root: &'a Node, value: &str) -> Option<&'a [Child]> {
    root.children
        .iter()
        .filter_map(Child::as_node)
        .find_map(|node| {
            if node.kind != VARIANT {
                return None;
            }
            let Value::String(when) = node.props.get("when")? else {
                return None;
            };
            when.split_whitespace()
                .any(|word| word == value)
                .then_some(node.children.as_slice())
        })
}

/// What a `<use>` places: the chosen variant's body, or the fragment body when it has no
/// variant parameter. An unresolved variant places nothing, because the body is not known.
pub fn placed_children<'a>(root: &'a Node, use_node: &Node) -> &'a [Child] {
    let Some(chosen) = choice(root, use_node) else {
        return &root.children[declaration_len(root)..];
    };
    chosen
        .value
        .and_then(|value| variant_body(root, value))
        .unwrap_or(&[])
}

/// Whether `list` holds an `<outlet>` of `name`. A variant without one drops that slot.
pub fn has_outlet(list: &[Child], name: &str) -> bool {
    let mut stack: Vec<&Node> = list.iter().filter_map(Child::as_node).collect();
    while let Some(node) = stack.pop() {
        if node.kind == OUTLET
            && matches!(node.props.get("name"), Some(Value::String(n)) if n == name)
        {
            return true;
        }
        if node.kind == VARIANT {
            continue;
        }
        let lists = node.slots.values().chain(std::iter::once(&node.children));
        stack.extend(lists.flatten().filter_map(Child::as_node));
    }
    false
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
    // Which body a use has is structure, so the variant parameter is a literal (W217).
    let variant_name = axis(root).map(|a| a.name.to_owned());
    for (name, kind) in params(root) {
        match kind {
            ParamKind::Value(mut def) => {
                if variant_name.as_deref() == Some(name.as_str()) {
                    def.bindable = Some(false);
                }
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
