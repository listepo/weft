//! The data schema of SPEC §10.5: a JSON Schema (2020-12) subset read into a closed shape, and a
//! pass that checks every binding of a document against it. Ported from packages/core/src/data.ts
//! with the same problems, codes and messages. It is a pass of its own rather than a
//! `ValidateOptions` field so that the option structs every binding constructs stay as they are.
//! The schema is untrusted: nothing here panics.

use indexmap::IndexMap;
use serde_json::Value as Json;

use crate::diagnostics::{Code, Diagnostic, Position, did_you_mean, one_of};
use crate::model::{Catalog, Child, Document, Node, PropDef, PropType, Value};
use crate::rules::{EACH, MAX_DEPTH, is_binding, is_loop_variable, universal_prop};
use crate::source::path_segment;

#[derive(Clone, Debug, PartialEq)]
pub enum DataSchema {
    Any,
    Never,
    Shape(Box<Shape>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// `None`: any JSON type.
    pub types: Option<Vec<String>>,
    pub properties: Option<IndexMap<String, DataSchema>>,
    /// What names outside `properties` lead to; `Never` closes the object.
    pub additional: DataSchema,
    pub items: Option<DataSchema>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataSchemaProblem {
    /// `W709` or `W710`.
    pub code: Code,
    /// JSON Pointer into the schema, "" for its root.
    pub pointer: String,
    pub message: String,
}

#[derive(Clone, Copy)]
pub struct DataCheckOptions<'a> {
    pub catalog: &'a Catalog,
    pub data: &'a DataSchema,
}

const TYPES: &[&str] = &[
    "array", "boolean", "integer", "null", "number", "object", "string",
];
const UNSUPPORTED: &[&str] = &[
    "$ref",
    "$dynamicRef",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "dependentSchemas",
    "prefixItems",
    "contains",
    "patternProperties",
    "propertyNames",
    "unevaluatedItems",
    "unevaluatedProperties",
];

fn is_index(s: &str) -> bool {
    s == "0"
        || (s.starts_with(|c: char| ('1'..='9').contains(&c))
            && s.chars().all(|c| c.is_ascii_digit()))
}

fn escape_pointer(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

fn at(pointer: &str) -> &str {
    if pointer.is_empty() {
        "the root of the data schema"
    } else {
        pointer
    }
}

pub fn compile_data_schema(json: &Json) -> (DataSchema, Vec<DataSchemaProblem>) {
    let mut problems = Vec::new();
    let schema = compile(json, "", 0, &mut problems);
    (schema, problems)
}

/// Compile problems as diagnostics, so `checkData` and a project file report the same codes.
pub fn data_schema_diagnostics(problems: &[DataSchemaProblem]) -> Vec<Diagnostic> {
    problems
        .iter()
        .map(|p| {
            let expected = if p.code == Code::W709 {
                "a JSON Schema 2020-12 object or boolean (SPEC §10.5)"
            } else {
                "\"type\", \"properties\", \"additionalProperties\" or \"items\""
            };
            let path = if p.pointer.is_empty() {
                "#".to_owned()
            } else {
                format!("#{}", p.pointer)
            };
            Diagnostic::new(p.code, path, p.message.clone(), expected)
        })
        .collect()
}

fn malformed(problems: &mut Vec<DataSchemaProblem>, pointer: String, message: String) {
    problems.push(DataSchemaProblem {
        code: Code::W709,
        pointer,
        message,
    });
}

fn compile(
    s: &Json,
    pointer: &str,
    depth: usize,
    problems: &mut Vec<DataSchemaProblem>,
) -> DataSchema {
    if depth > MAX_DEPTH {
        let message = format!(
            "Schemas nest deeper than {MAX_DEPTH} levels at {}.",
            at(pointer)
        );
        malformed(problems, pointer.to_owned(), message);
        return DataSchema::Any;
    }
    let s = match s {
        Json::Bool(true) => return DataSchema::Any,
        Json::Bool(false) => return DataSchema::Never,
        Json::Object(s) => s,
        _ => {
            let message = format!(
                "The schema at {} is neither an object nor a boolean.",
                at(pointer)
            );
            malformed(problems, pointer.to_owned(), message);
            return DataSchema::Any;
        }
    };
    let mut unsupported = false;
    for key in s.keys() {
        if !UNSUPPORTED.contains(&key.as_str()) {
            continue;
        }
        unsupported = true;
        let pointer = format!("{pointer}/{}", escape_pointer(key));
        let message =
            format!("\"{key}\" at {pointer} is not supported; that schema accepts any data.");
        problems.push(DataSchemaProblem {
            code: Code::W710,
            pointer,
            message,
        });
    }
    if unsupported {
        return DataSchema::Any;
    }

    let mut types = None;
    if let Some(t) = s.get("type") {
        let list: Option<Vec<String>> = match t {
            Json::String(t) => Some(vec![t.clone()]),
            Json::Array(items) => items
                .iter()
                .map(|i| i.as_str().map(str::to_owned))
                .collect(),
            _ => None,
        };
        match list {
            Some(list) if list.iter().all(|t| TYPES.contains(&t.as_str())) => types = Some(list),
            _ => malformed(
                problems,
                format!("{pointer}/type"),
                format!("\"type\" at {pointer}/type names no JSON Schema type."),
            ),
        }
    }
    let mut properties = None;
    if let Some(p) = s.get("properties") {
        match p {
            Json::Object(p) => {
                let mut map = IndexMap::new();
                for (name, sub) in p {
                    let inner = format!("{pointer}/properties/{}", escape_pointer(name));
                    map.insert(name.clone(), compile(sub, &inner, depth + 1, problems));
                }
                properties = Some(map);
            }
            _ => malformed(
                problems,
                format!("{pointer}/properties"),
                format!("\"properties\" at {pointer}/properties is not an object."),
            ),
        }
    }
    // SPEC §10.5: listed properties are all the properties, unlike JSON Schema's open default.
    let additional = match s.get("additionalProperties") {
        None if properties.is_none() => DataSchema::Any,
        None => DataSchema::Never,
        Some(extra) => compile(
            extra,
            &format!("{pointer}/additionalProperties"),
            depth + 1,
            problems,
        ),
    };
    let items = s
        .get("items")
        .map(|i| compile(i, &format!("{pointer}/items"), depth + 1, problems));
    DataSchema::Shape(Box::new(Shape {
        types,
        properties,
        additional,
        items,
    }))
}

fn allows(s: &DataSchema, kind: &str) -> bool {
    match s {
        DataSchema::Any => true,
        DataSchema::Never => false,
        DataSchema::Shape(s) => s.types.as_ref().is_none_or(|t| t.iter().any(|t| t == kind)),
    }
}

/// One path step; `None` when the schema does not declare it.
fn step<'s>(s: &'s DataSchema, segment: &str) -> Option<&'s DataSchema> {
    let shape = match s {
        DataSchema::Any => return Some(&DataSchema::Any),
        DataSchema::Never => return None,
        DataSchema::Shape(shape) => shape,
    };
    if is_index(segment) && allows(s, "array") && (shape.types.is_some() || shape.items.is_some()) {
        return Some(shape.items.as_ref().unwrap_or(&DataSchema::Any));
    }
    if !allows(s, "object") {
        return None;
    }
    let next = shape
        .properties
        .as_ref()
        .and_then(|p| p.get(segment))
        .unwrap_or(&shape.additional);
    (*next != DataSchema::Never).then_some(next)
}

fn names(s: &DataSchema) -> Vec<String> {
    match s {
        DataSchema::Shape(shape) => shape
            .properties
            .iter()
            .flatten()
            .filter(|(_, v)| **v != DataSchema::Never)
            .map(|(k, _)| k.clone())
            .collect(),
        _ => vec![],
    }
}

fn type_text(s: &DataSchema) -> String {
    match s {
        DataSchema::Shape(shape) => shape
            .types
            .as_ref()
            .map_or_else(|| "any type".to_owned(), |t| t.join(" or ")),
        _ => "any type".to_owned(),
    }
}

fn declared_types(s: &DataSchema) -> Option<&[String]> {
    match s {
        DataSchema::Shape(shape) => shape.types.as_deref(),
        _ => None,
    }
}

/// JSON types a plain binding may deliver to a prop of this definition (SPEC §10.5).
fn accepted(def: &PropDef) -> &'static [&'static str] {
    match def.kind {
        PropType::String => &["string", "number", "integer"],
        PropType::Number => &["number", "integer"],
        PropType::Boolean => &["boolean"],
        PropType::Enum | PropType::Token => &["string"],
    }
}

struct Checker<'a> {
    catalog: &'a Catalog,
    data: &'a DataSchema,
    out: Vec<Diagnostic>,
}

struct Where {
    path: String,
    pos: Option<Position>,
}

impl Where {
    fn diagnostic(&self, code: Code, message: String, expected: String) -> Diagnostic {
        Diagnostic::new(code, self.path.clone(), message, expected).pos(self.pos)
    }
}

type Vars = Vec<(String, DataSchema)>;

impl Checker<'_> {
    /// The schema at the end of `path`, or `None` when it is unknown or was just reported.
    fn resolve(&mut self, path: &str, vars: &Vars, at: &Where) -> Option<DataSchema> {
        // A malformed path or an unknown loop variable is the validator's to report (W214, W305).
        if !is_binding(path) {
            return None;
        }
        let (mut schema, segments, mut walked): (&DataSchema, Vec<&str>, String) =
            if let Some(rest) = path.strip_prefix("$.") {
                (self.data, rest.split('.').collect(), "$".to_owned())
            } else {
                let mut parts = path.get(1..).unwrap_or_default().split('.');
                let variable = parts.next().unwrap_or_default();
                let found = vars.iter().rev().find(|(name, _)| name == variable)?;
                (&found.1, parts.collect(), format!("${variable}"))
            };
        for segment in segments {
            let Some(next) = step(schema, segment) else {
                let declared = names(schema);
                let expected = if !declared.is_empty() {
                    one_of(&declared)
                } else if allows(schema, "array") && declared_types(schema).is_some() {
                    "an array index such as 0".to_owned()
                } else {
                    "a path the data schema declares".to_owned()
                };
                let d = at
                    .diagnostic(
                        Code::W315,
                        format!("The data schema does not declare \"{segment}\" in {walked}."),
                        expected,
                    )
                    .got(path)
                    .hint_opt(did_you_mean(segment, &declared));
                self.out.push(d);
                return None;
            };
            schema = next;
            walked = format!("{walked}.{segment}");
        }
        Some(schema.clone())
    }

    fn visit_list(&mut self, list: &[Child], list_path: &str, vars: &Vars, depth: usize) {
        for (index, child) in list.iter().enumerate() {
            if let Child::Node(child) = child {
                let segment = path_segment(&child.kind, child.id.as_deref(), Some(index));
                self.visit(child, &format!("{list_path}/{segment}"), vars, depth + 1);
            }
        }
    }

    fn visit(&mut self, node: &Node, path: &str, vars: &Vars, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let each = node.kind == EACH;
        let component = if each {
            None
        } else {
            self.catalog.def_of(node)
        };
        let mut item = DataSchema::Any;
        for (name, value) in &node.props {
            let Value::Bind { bind, not } = value else {
                continue;
            };
            let source = node.source.0.as_deref();
            let at = Where {
                path: format!("{path}/@{name}"),
                pos: source.and_then(|s| s.attrs.get(name).copied().or(Some(s.pos))),
            };
            let Some(found) = self.resolve(bind, vars, &at) else {
                continue;
            };
            if each {
                if name != "in" || *not {
                    continue;
                }
                if !allows(&found, "array") {
                    let text = type_text(&found);
                    let d = at
                        .diagnostic(
                            Code::W316,
                            format!("<each> repeats an array; the data at {bind} is {text}."),
                            "array".to_owned(),
                        )
                        .got(text);
                    self.out.push(d);
                } else if let DataSchema::Shape(shape) = &found
                    && let Some(items) = &shape.items
                {
                    item = items.clone();
                }
                continue;
            }
            // A negated binding tests emptiness, which every type has (SPEC §10.5).
            if *not {
                continue;
            }
            let def = component
                .and_then(|c| c.prop(name))
                .or_else(|| universal_prop(name));
            let (Some(def), Some(types)) = (def, declared_types(&found)) else {
                continue;
            };
            let takes = accepted(def);
            if !types.iter().any(|t| takes.contains(&t.as_str())) {
                let text = type_text(&found);
                let wanted = takes.join(" or ");
                let d = at
                    .diagnostic(
                        Code::W316,
                        format!("The data at {bind} is {text}; this attribute takes {wanted}."),
                        wanted,
                    )
                    .got(text);
                self.out.push(d);
            }
        }
        let inner_vars: Vars;
        let mut vars = vars;
        if each
            && let Some(Value::String(variable)) = node.props.get("as")
            && is_loop_variable(variable)
        {
            let mut extended = vars.clone();
            extended.push((variable.clone(), item));
            inner_vars = extended;
            vars = &inner_vars;
        }
        self.visit_list(&node.children, path, vars, depth);
        for (name, list) in &node.slots {
            self.visit_list(list, &format!("{path}/slot[{name}]"), vars, depth);
        }
    }
}

/// Checks every binding of `document` against the data schema (SPEC §10.5): `W315` for a path
/// the schema does not declare, `W316` for data of a type the attribute does not take.
pub fn check_data(document: &Document, options: &DataCheckOptions<'_>) -> Vec<Diagnostic> {
    let mut checker = Checker {
        catalog: options.catalog,
        data: options.data,
        out: vec![],
    };
    let root = &document.root;
    let path = format!("/{}", path_segment(&root.kind, root.id.as_deref(), None));
    checker.visit(root, &path, &vec![], 1);
    checker.out
}

/// [`check_data`] for canonical JSON input. Input that `validate` rejects for its shape (`W200`)
/// gives no diagnostics here: the shape problems are already reported.
pub fn check_data_json(input: &Json, options: &DataCheckOptions<'_>) -> Vec<Diagnostic> {
    if crate::validate::json_exceeds_depth(input)
        || !crate::shape::document_issues(input).is_empty()
    {
        return vec![];
    }
    check_data(&crate::shape::to_document(input), options)
}
