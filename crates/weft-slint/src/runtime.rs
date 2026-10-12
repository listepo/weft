//! The generated source at run time (feature `runtime`): compiled by `slint-interpreter` and filled
//! with sample data. The data is untrusted input, as a project's `render.data` is, so every
//! mismatch is a reported problem and nothing panics. No backend or renderer comes with it: the
//! caller supplies the platform, as the editor's host and the screenshot tests do.

use std::collections::BTreeMap;
use std::task::{Context, Poll, Waker};

use serde_json::Value as Json;
use slint_interpreter::{
    Compiler, ComponentDefinition, ComponentInstance, DiagnosticLevel, SharedString, Struct, Value,
};

/// The generated source's one exported component, or the compiler's error messages.
pub fn compile(source: &str, style: &str) -> Result<ComponentDefinition, Vec<String>> {
    let mut compiler = Compiler::default();
    compiler.set_style(style.into());
    let mut build =
        std::pin::pin!(compiler.build_from_source(source.into(), "screen.slint".into()));
    let mut cx = Context::from_waker(Waker::noop());
    // Without a file loader the build never waits, so polling once is enough; the loop guards it.
    let result = loop {
        if let Poll::Ready(result) = build.as_mut().poll(&mut cx) {
            break result;
        }
    };
    let errors: Vec<String> = result
        .diagnostics()
        .filter(|d| d.level() == DiagnosticLevel::Error)
        .map(|d| d.message().to_owned())
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut components: Vec<_> = result.components().collect();
    match (components.pop(), components.is_empty()) {
        (Some(component), true) => Ok(component),
        _ => Err(vec![format!(
            "expected one exported component, found {}",
            result.component_names().count()
        )]),
    }
}

/// Sets each `in-out` property of the generated `source` that `data` has a value for, and returns
/// the problems: a value of the wrong type, or a type the generator does not print. The other
/// properties are still set.
pub fn apply_data(instance: &ComponentInstance, source: &str, data: &Json) -> Vec<String> {
    let mut flat = BTreeMap::new();
    flatten(data, "", &mut flat);
    let mut problems = vec![];
    for (name, ty) in properties(source) {
        let Some(json) = lookup(&flat, name) else {
            continue;
        };
        let set = parse_ty(ty)
            .and_then(|parsed| json_to_value(&parsed, json, name))
            .and_then(|value| {
                instance
                    .set_property(name, value)
                    .map_err(|e| e.to_string())
            });
        if let Err(problem) = set {
            problems.push(format!("{name} <{ty}>: {problem}"));
        }
    }
    problems
}

enum SlintTy {
    String,
    Bool,
    Int,
    Float,
    StringList,
    StructList(Vec<(String, Leaf)>),
}

enum Leaf {
    String,
    Bool,
    Int,
    Float,
}

/// The data properties the generator prints: `in-out property <type> name;`.
fn properties(source: &str) -> Vec<(&str, &str)> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("in-out property <")?;
            let (ty, name) = rest.split_once("> ")?;
            Some((name.trim_end_matches(';'), ty))
        })
        .collect()
}

fn leaf(ty: &str) -> Result<Leaf, String> {
    match ty {
        "string" => Ok(Leaf::String),
        "bool" => Ok(Leaf::Bool),
        "int" => Ok(Leaf::Int),
        "float" => Ok(Leaf::Float),
        other => Err(format!("unhandled field type {other}")),
    }
}

fn parse_ty(ty: &str) -> Result<SlintTy, String> {
    match ty {
        "string" => Ok(SlintTy::String),
        "bool" => Ok(SlintTy::Bool),
        "int" => Ok(SlintTy::Int),
        "float" => Ok(SlintTy::Float),
        "[string]" => Ok(SlintTy::StringList),
        other if other.starts_with("[{") && other.ends_with("}]") => {
            let fields = other[2..other.len() - 2]
                .split(',')
                .map(|part| {
                    let (name, ty) = part
                        .trim()
                        .split_once(':')
                        .ok_or_else(|| format!("unhandled Slint type {other}"))?;
                    Ok((name.trim().to_owned(), leaf(ty.trim())?))
                })
                .collect::<Result<_, String>>()?;
            Ok(SlintTy::StructList(fields))
        }
        other => Err(format!("unhandled Slint type {other}")),
    }
}

/// `$.stats.velocity` is the property `stats-velocity`, so a nested object flattens on `-`.
/// An array stays whole: it is the model, not another path.
fn flatten(value: &Json, prefix: &str, out: &mut BTreeMap<String, Json>) {
    if let Json::Object(map) = value
        && !map.is_empty()
    {
        for (key, child) in map {
            let name = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}-{key}")
            };
            flatten(child, &name, out);
        }
        return;
    }
    if !prefix.is_empty() {
        out.insert(prefix.to_owned(), value.clone());
    }
}

fn lookup<'a>(flat: &'a BTreeMap<String, Json>, name: &str) -> Option<&'a Json> {
    flat.get(name).or_else(|| {
        // A path Slint's `Window` already owns is stored as `<name>-data`.
        name.strip_suffix("-data").and_then(|bare| flat.get(bare))
    })
}

fn missing(ty: &Leaf) -> Json {
    match ty {
        Leaf::String => Json::String(String::new()),
        Leaf::Bool => Json::Bool(false),
        Leaf::Int | Leaf::Float => Json::from(0),
    }
}

fn leaf_value(ty: &Leaf, json: &Json, at: &str) -> Result<Value, String> {
    Ok(match ty {
        // A number field is still a LineEdit, so its property is a string and the sample may be a number.
        Leaf::String => Value::String(SharedString::from(match json {
            Json::String(text) => text.clone(),
            Json::Number(number) => number.to_string(),
            Json::Bool(value) => value.to_string(),
            _ => return Err(format!("{at}: expected text")),
        })),
        // A bool property is often a text path read for truthiness (`!= ""`), and the sample
        // still holds the text, as `selection: ""`.
        Leaf::Bool => Value::Bool(match json {
            Json::Bool(value) => *value,
            Json::String(text) => !text.is_empty(),
            Json::Number(number) => number.as_f64().unwrap_or(0.0) != 0.0,
            _ => return Err(format!("{at}: expected a bool")),
        }),
        Leaf::Int | Leaf::Float => Value::Number(
            json.as_f64()
                .ok_or_else(|| format!("{at}: expected a number"))?,
        ),
    })
}

fn model(items: Vec<Value>) -> Value {
    Value::Model(items.as_slice().into())
}

/// A `[string]` model is sometimes fed by objects whose one text field is the row, as `cities`
/// is `[{ name }]` while the combo box model is `[string]`.
fn string_item(item: &Json, at: &str) -> Result<Value, String> {
    match item {
        Json::Object(map) => {
            let texts: Vec<_> = map.values().filter(|value| value.is_string()).collect();
            match texts[..] {
                [text] => leaf_value(&Leaf::String, text, at),
                _ => Err(format!("{at}: a [string] row needs one text field")),
            }
        }
        other => leaf_value(&Leaf::String, other, at),
    }
}

fn json_to_value(ty: &SlintTy, json: &Json, at: &str) -> Result<Value, String> {
    let rows = || {
        json.as_array()
            .ok_or_else(|| format!("{at}: expected an array"))
    };
    match ty {
        SlintTy::String => leaf_value(&Leaf::String, json, at),
        SlintTy::Bool => leaf_value(&Leaf::Bool, json, at),
        SlintTy::Int => leaf_value(&Leaf::Int, json, at),
        SlintTy::Float => leaf_value(&Leaf::Float, json, at),
        SlintTy::StringList => Ok(model(
            rows()?
                .iter()
                .map(|item| string_item(item, at))
                .collect::<Result<_, _>>()?,
        )),
        SlintTy::StructList(fields) => {
            let items = rows()?
                .iter()
                .map(|row| {
                    let mut item = Struct::default();
                    for (name, ty) in fields {
                        let value = match row.get(name) {
                            Some(v) => leaf_value(ty, v, name)?,
                            None => leaf_value(ty, &missing(ty), name)?,
                        };
                        item.set_field(name.clone(), value);
                    }
                    Ok(Value::Struct(item))
                })
                .collect::<Result<_, String>>()?;
            Ok(model(items))
        }
    }
}
