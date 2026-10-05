//! The model built from sample data: `extension <Model> { static var sample }`, which the file's
//! `#Preview` shows. Each value is read into the Swift type inferred for its path with the
//! readings every Weft renderer shares (`weft_core::text`, `truthy`), so a preview shows what
//! the reference renderer would. The importer reads views and action enums only, so none of this
//! reaches a document read back.

use std::collections::HashMap;

use serde_json::Value as Json;
use weft_core::{controls, js_round, text, truthy};

use crate::data::{Shapes, Ty};
use crate::swift::{self, member, string_literal};

pub fn extension(
    model: &str,
    shapes: &Shapes,
    names: &HashMap<usize, String>,
    data: &Json,
) -> Vec<String> {
    let printer = Printer {
        model,
        shapes,
        names,
    };
    let mut value = printer.value(&Ty::Struct(0), Some(data));
    if let Some(first) = value.first_mut() {
        *first = format!("        {first}");
    }
    let mut out = vec![
        format!("extension {model} {{"),
        "    /// The sample data the screen was generated with.".to_owned(),
        format!("    static var sample: {model} {{"),
    ];
    out.extend(
        value
            .into_iter()
            .enumerate()
            .map(|(i, l)| if i == 0 { l } else { format!("        {l}") }),
    );
    out.push("    }".to_owned());
    out.push("}".to_owned());
    out
}

struct Printer<'a> {
    model: &'a str,
    shapes: &'a Shapes,
    names: &'a HashMap<usize, String>,
}

impl Printer<'_> {
    /// A Swift expression of type `ty` for `value`, one line or several; lines after the first
    /// are indented relative to the first. Data of another shape reads as the type's default.
    fn value(&self, ty: &Ty, value: Option<&Json>) -> Vec<String> {
        match ty {
            Ty::String => vec![string_literal(&text(value))],
            Ty::Bool => vec![truthy(value).to_string()],
            Ty::Int => vec![int(value).to_string()],
            Ty::Double => vec![swift::number_literal(double(value))],
            Ty::Struct(at) => {
                let name = if *at == 0 {
                    self.model
                } else {
                    self.names.get(at).map_or("", String::as_str)
                };
                let fields = &self.shapes.nodes[*at].fields;
                let object = value.and_then(Json::as_object);
                let args: Vec<Vec<String>> = fields
                    .iter()
                    .map(|(field, &child)| {
                        let inner = self.value(
                            self.shapes.type_at(child),
                            object.and_then(|o| o.get(field)),
                        );
                        labelled(&member(field), inner)
                    })
                    .collect();
                wrap(&format!("{name}("), args, ")")
            }
            Ty::Array(item) => {
                let items = value
                    .and_then(Json::as_array)
                    .map_or(&[][..], Vec::as_slice);
                let elements = items.iter().map(|v| self.value(item, Some(v))).collect();
                wrap("[", elements, "]")
            }
        }
    }
}

/// `label: value`, the label on the value's first line.
fn labelled(label: &str, mut value: Vec<String>) -> Vec<String> {
    if let Some(first) = value.first_mut() {
        *first = format!("{label}: {first}");
    }
    value
}

/// `open` + the parts, comma-separated, one per line + `close`; `open` + `close` when empty.
fn wrap(open: &str, parts: Vec<Vec<String>>, close: &str) -> Vec<String> {
    if parts.is_empty() {
        return vec![format!("{open}{close}")];
    }
    let mut out = vec![open.to_owned()];
    let last = parts.len() - 1;
    for (i, mut part) in parts.into_iter().enumerate() {
        if i < last
            && let Some(end) = part.last_mut()
        {
            end.push(',');
        }
        out.extend(part.into_iter().map(|l| format!("    {l}")));
    }
    out.push(close.to_owned());
    out
}

/// A number as Swift's `Int`: rounded as the renderer rounds whole-number props, and held to
/// `Int`'s range; anything but a number is 0.
/// A number as the number controls read it: a number, or text written as one.
fn double(value: Option<&Json>) -> f64 {
    let number = value.and_then(Json::as_f64);
    let text = value.and_then(Json::as_str);
    controls::numeric(number, text).unwrap_or(0.0)
}

fn int(value: Option<&Json>) -> i64 {
    match value {
        Some(Json::Number(n)) => n.as_i64().unwrap_or_else(|| {
            // Beyond `i64`, or a fraction: `as` saturates at the bounds.
            n.as_f64().map_or(0, |f| js_round(f) as i64)
        }),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_become_ints_as_the_renderer_rounds_them() {
        assert_eq!(int(Some(&json!(2.5))), 3);
        assert_eq!(int(Some(&json!(-2.5))), -2);
        assert_eq!(int(Some(&json!(1e300))), i64::MAX);
        assert_eq!(int(Some(&json!(u64::MAX))), i64::MAX);
        assert_eq!(int(Some(&json!("7"))), 0);
        assert_eq!(int(None), 0);
    }

    #[test]
    fn lists_wrap_one_part_per_line() {
        let parts = vec![vec!["a".to_owned()], vec!["b(".to_owned(), ")".to_owned()]];
        assert_eq!(
            wrap("[", parts, "]"),
            ["[", "    a,", "    b(", "    )", "]"]
        );
        assert_eq!(wrap("X(", vec![], ")"), ["X()"]);
    }
}
