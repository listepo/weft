//! The JSON Schema of the canonical documents a catalog admits (SPEC §3.1), for writers whose
//! decoder takes a schema. It is a projection of validation, built from the same catalog and the
//! validator's own table of universal attributes, so the two cannot drift; the validator keeps the
//! last word on everything the schema cannot say.

use std::collections::BTreeMap;

use serde_json::{Map as Object, Value as Json, json};
use weft_core::{Catalog, ComponentDef, Content, PropDef, PropType, WEFT_VERSION, universal_props};

const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const EACH: &str = "each";

// The grammars of SPEC §2.1–§2.2 and §4.3, as ECMA-262 patterns without look-around.
const ID: &str = "^[A-Za-z][A-Za-z0-9_-]*$";
const BINDING: &str = "^\\$(?:(?:\\.(?:[A-Za-z_][A-Za-z0-9_]*|0|[1-9][0-9]*))+|[A-Za-z_][A-Za-z0-9_]*(?:\\.(?:[A-Za-z_][A-Za-z0-9_]*|0|[1-9][0-9]*))*)$";
const TOKEN: &str = "^[A-Za-z0-9_-]+(?:\\.[A-Za-z0-9_-]+)*$";
const ACTION: &str = "^[a-z][A-Za-z0-9]*(?:\\.[a-z][A-Za-z0-9]*)*$";
const LOOP_VARIABLE: &str = "^[a-z][A-Za-z0-9]*$";
/// The versions a reader of `WEFT_VERSION` reads without a diagnostic: a 0.1 document is a valid
/// 0.2 document, and the corpus stays at 0.1.
const VERSIONS: [&str; 2] = ["0.1", WEFT_VERSION];

/// Room for the project values a schema may later be narrowed with (token and action names);
/// for now the schema describes the catalog only.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct DocumentSchemaOptions {}

fn reference(name: &str) -> Json {
    json!({ "$ref": format!("#/$defs/{name}") })
}

/// `false` admits nothing, and `anyOf` must not be empty.
fn one_of_these(mut alternatives: Vec<Json>) -> Json {
    match alternatives.len() {
        0 => Json::Bool(false),
        1 => alternatives.remove(0),
        _ => json!({ "anyOf": alternatives }),
    }
}

fn closed(properties: Object<String, Json>, required: Vec<&str>) -> Json {
    let mut schema = Object::new();
    schema.insert("type".into(), "object".into());
    schema.insert("properties".into(), properties.into());
    if !required.is_empty() {
        schema.insert("required".into(), json!(required));
    }
    schema.insert("additionalProperties".into(), false.into());
    schema.into()
}

struct Generator<'a> {
    catalog: &'a Catalog,
    /// Kinds in name order: equal catalogs must give equal bytes whatever their key order.
    kinds: Vec<(&'a str, &'a ComponentDef)>,
    defs: BTreeMap<String, Json>,
}

impl<'a> Generator<'a> {
    fn is_root(def: &ComponentDef) -> bool {
        def.root == Some(true)
    }

    /// The elements a list of `parent` admits (SPEC §3.1), with the list's own `each` entry,
    /// registered under `each_name`, when the list admits `each`.
    fn admitted(
        &mut self,
        parent: &str,
        allowed: Option<&Vec<String>>,
        each_name: &str,
    ) -> Vec<Json> {
        let placed_in = |def: &ComponentDef| {
            def.allowed_parents
                .as_ref()
                .is_none_or(|parents| parents.iter().any(|p| p == parent))
        };
        let mut out = Vec::new();
        let admits_each = match allowed {
            None => {
                if self.defs.contains_key("Node") {
                    out.push(reference("Node"));
                }
                out.extend(
                    self.kinds
                        .iter()
                        .filter(|(_, d)| {
                            !Self::is_root(d) && d.allowed_parents.is_some() && placed_in(d)
                        })
                        .map(|(k, _)| reference(k)),
                );
                true
            }
            Some(list) => {
                out.extend(
                    self.kinds
                        .iter()
                        .filter(|(k, d)| {
                            list.iter().any(|a| a == k) && !Self::is_root(d) && placed_in(d)
                        })
                        .map(|(k, _)| reference(k)),
                );
                list.iter().any(|a| a == EACH)
            }
        };
        if admits_each {
            out.push(reference(each_name));
            // `<each>` is transparent: its children answer to the list that holds it (SPEC §4.3).
            let children =
                json!({ "type": "array", "minItems": 1, "items": one_of_these(out.clone()) });
            let mut props = Object::new();
            props.insert(
                "as".into(),
                json!({ "type": "string", "pattern": LOOP_VARIABLE }),
            );
            props.insert("in".into(), reference("Binding"));
            let mut node = Object::new();
            node.insert("kind".into(), json!({ "const": EACH }));
            node.insert("id".into(), reference("Id"));
            node.insert("props".into(), closed(props, vec!["as", "in"]));
            node.insert("children".into(), children);
            let mut each = closed(node, vec!["kind", "id", "props", "children"]);
            each["description"] = "Repeats its elements once per item of the array `in` is bound to, with the item named by `as` (SPEC §4.3).".into();
            self.defs.insert(each_name.to_owned(), each);
        }
        out
    }

    fn value(def: &PropDef, values: Option<&Vec<String>>) -> Json {
        let mut forms = Vec::new();
        match def.kind {
            PropType::String => forms.push(json!({ "type": "string" })),
            PropType::Boolean => forms.push(json!({ "type": "boolean" })),
            PropType::Enum => forms.push(
                json!({ "enum": values.or(def.values.as_ref()).cloned().unwrap_or_default() }),
            ),
            PropType::Token => forms.push(reference("Token")),
            PropType::Number => {
                let integer = def.integer == Some(true);
                let mut number = Object::new();
                number.insert(
                    "type".into(),
                    if integer { "integer" } else { "number" }.into(),
                );
                if let Some(min) = def.min {
                    number.insert("minimum".into(), json!(min));
                }
                if let Some(max) = def.max {
                    number.insert("maximum".into(), json!(max));
                }
                forms.push(number.into());
            }
        }
        if def.bindable != Some(false) {
            // A negated binding is read-only and boolean (`W218`).
            let negatable = def.kind == PropType::Boolean && def.writable != Some(true);
            forms.push(reference(if negatable {
                "NegatableBinding"
            } else {
                "Binding"
            }));
        }
        let mut schema = one_of_these(forms);
        if let Json::Object(o) = &mut schema {
            o.insert("description".into(), def.description.clone().into());
        }
        schema
    }

    fn props(kind_def: &ComponentDef, root: bool) -> (Json, bool) {
        let mut props: BTreeMap<&str, Json> = BTreeMap::new();
        for (name, def) in universal_props() {
            match name {
                // A catalog component already has its role (`W209`).
                "role" => {}
                "state" => {
                    if let Some(states) = kind_def.states.as_ref().filter(|s| !s.is_empty()) {
                        props.insert(name, Self::value(def, Some(states)));
                    }
                }
                // Shared entries: written once instead of once per kind.
                _ => {
                    props.insert(name, reference(&format!("universal:{name}")));
                }
            }
        }
        let mut required = Vec::new();
        for (name, def) in kind_def.props.iter().flatten() {
            // The root's version is `Document.weft`, never a prop (`W200`).
            if name == "role" || (root && name == "weft") {
                continue;
            }
            props.insert(name, Self::value(def, None));
            if def.required == Some(true) {
                required.push(name.as_str());
            }
        }
        if kind_def.requires_label == Some(true) && !required.contains(&"label") {
            required.push("label");
        }
        // Prop names are ASCII, so byte order is the UTF-16 order canonical JSON sorts keys by.
        required.sort_unstable();
        let any_required = !required.is_empty();
        let properties = props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        (closed(properties, required), any_required)
    }

    fn kind(&mut self, kind: &str, def: &ComponentDef) -> Json {
        let (props, props_required) = Self::props(def, Self::is_root(def));
        let mut node = Object::new();
        node.insert("kind".into(), json!({ "const": kind }));
        node.insert("id".into(), reference("Id"));
        node.insert("props".into(), props);
        let mut required = vec!["kind", "id"];
        if props_required {
            required.push("props");
        }
        if let Some(events) = def.events.as_ref().filter(|e| !e.is_empty()) {
            let mut sorted: Vec<&String> = events.iter().collect();
            sorted.sort_unstable();
            let on = sorted
                .into_iter()
                .map(|e| (e.clone(), reference("Action")))
                .collect();
            node.insert("on".into(), closed(on, vec![]));
        }
        if let Some(slots) = def.slots.as_ref().filter(|s| !s.is_empty()) {
            let mut sorted: Vec<(&String, _)> = slots.iter().collect();
            sorted.sort_unstable_by_key(|(name, _)| *name);
            let mut properties = Object::new();
            let mut required_slots = Vec::new();
            for (name, slot) in sorted {
                let items = self.admitted(
                    kind,
                    slot.allowed_children.as_ref(),
                    &format!("each:{kind}:{name}"),
                );
                let schema = json!({ "type": "array", "items": one_of_these(items), "description": slot.description });
                properties.insert(name.clone(), schema);
                if slot.required == Some(true) {
                    required_slots.push(name.as_str());
                }
            }
            if !required_slots.is_empty() {
                required.push("slots");
            }
            node.insert("slots".into(), closed(properties, required_slots));
        }
        if def.content != Content::None {
            let mut items = Vec::new();
            if matches!(def.content, Content::Text | Content::Mixed) {
                items.push(json!({ "type": "string" }));
            }
            if matches!(def.content, Content::Nodes | Content::Mixed) {
                items.extend(self.admitted(
                    kind,
                    def.allowed_children.as_ref(),
                    &format!("each:{kind}"),
                ));
            }
            node.insert(
                "children".into(),
                json!({ "type": "array", "items": one_of_these(items) }),
            );
        }
        let mut schema = closed(node, required);
        schema["description"] = def.description.clone().into();
        schema
    }
}

/// The JSON Schema (2020-12) of the canonical JSON documents `catalog` admits (SPEC §3.1). Equal
/// catalogs give byte-equal schemas.
pub fn document_schema(catalog: &Catalog, _options: &DocumentSchemaOptions) -> Json {
    let mut kinds: Vec<(&str, &ComponentDef)> = catalog
        .components
        .iter()
        .map(|(k, d)| (k.as_str(), d))
        .collect();
    kinds.sort_unstable_by_key(|(k, _)| *k);
    let mut g = Generator {
        catalog,
        kinds,
        defs: BTreeMap::new(),
    };
    g.defs
        .insert("Id".into(), json!({ "type": "string", "pattern": ID }));
    g.defs.insert(
        "Action".into(),
        json!({ "type": "string", "pattern": ACTION }),
    );
    let path = json!({ "type": "string", "pattern": BINDING });
    g.defs.insert(
        "Binding".into(),
        closed(
            Object::from_iter([("bind".into(), path.clone())]),
            vec!["bind"],
        ),
    );
    g.defs.insert(
        "NegatableBinding".into(),
        closed(
            Object::from_iter([
                ("bind".into(), path),
                ("not".into(), json!({ "const": true })),
            ]),
            vec!["bind"],
        ),
    );
    g.defs.insert(
        "Token".into(),
        closed(
            Object::from_iter([(
                "token".into(),
                json!({ "type": "string", "pattern": TOKEN }),
            )]),
            vec!["token"],
        ),
    );
    for (name, def) in universal_props().filter(|(n, _)| !matches!(*n, "role" | "state")) {
        g.defs
            .insert(format!("universal:{name}"), Generator::value(def, None));
    }
    let free: Vec<Json> = g
        .kinds
        .iter()
        .filter(|(_, d)| !Generator::is_root(d) && d.allowed_parents.is_none())
        .map(|(k, _)| reference(k))
        .collect();
    if !free.is_empty() {
        g.defs.insert("Node".into(), one_of_these(free));
    }
    for (kind, def) in g.kinds.clone() {
        let schema = g.kind(kind, def);
        g.defs.insert(kind.to_owned(), schema);
    }
    let roots: Vec<Json> = g
        .kinds
        .iter()
        .filter(|(_, d)| Generator::is_root(d))
        .map(|(k, _)| reference(k))
        .collect();
    let root = if roots.is_empty() {
        one_of_these(g.kinds.iter().map(|(k, _)| reference(k)).collect())
    } else {
        one_of_these(roots)
    };
    let mut properties = Object::new();
    properties.insert("weft".into(), json!({ "enum": VERSIONS }));
    properties.insert("root".into(), root);
    let mut schema = Object::new();
    schema.insert("$schema".into(), DIALECT.into());
    schema.insert(
        "description".into(),
        format!(
            "A Weft {WEFT_VERSION} document in canonical JSON (SPEC §3) for catalog {} {}. Checks the validator makes beyond this schema are listed in SPEC §3.1.",
            g.catalog.name, g.catalog.version
        )
        .into(),
    );
    if let Json::Object(document) = closed(properties, vec!["weft", "root"]) {
        schema.extend(document);
    }
    schema.insert(
        "$defs".into(),
        g.defs.into_iter().collect::<Object<_, _>>().into(),
    );
    schema.into()
}
