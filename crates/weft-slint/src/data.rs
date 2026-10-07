//! The component's data: one `in-out property` per root data path that a binding reads, typed
//! from the props that read it, and the Slint expression that reads it as each prop needs it. Kept
//! apart from the generator so the typing rules can be tested on their own.

use indexmap::IndexMap;
use weft_core::{Catalog, Child, Node, PropType, Value, universal_prop};

use crate::Unsupported;
use crate::names::{clash_key, identifier};

/// Names a data property cannot take: the component's `perform` callback and the properties
/// every Slint `Window` already declares, which a component may not declare again.
const TAKEN: &[&str] = &[
    "perform",
    "title",
    "icon",
    "background",
    "always-on-top",
    "full-screen",
    "no-frame",
    "resize-border-width",
    "default-font-family",
    "default-font-size",
    "default-font-weight",
    "width",
    "height",
    "x",
    "y",
    "z",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "preferred-width",
    "preferred-height",
    "horizontal-stretch",
    "vertical-stretch",
    "opacity",
    "visible",
    "cache-rendering-hint",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Str,
    Bool,
    Float,
    Int,
}

impl Ty {
    pub fn slint(self) -> &'static str {
        match self {
            Ty::Str => "string",
            Ty::Bool => "bool",
            Ty::Float => "float",
            Ty::Int => "int",
        }
    }
}

/// The Slint type a prop holds, and whether user input writes it back.
pub fn prop_type(catalog: &Catalog, kind: &str, name: &str) -> (Ty, bool) {
    let def = catalog
        .components
        .get(kind)
        .and_then(|c| c.prop(name))
        .or_else(|| universal_prop(name));
    let Some(def) = def else {
        return (Ty::Str, false);
    };
    let ty = match def.kind {
        PropType::Boolean => Ty::Bool,
        // `SpinBox` holds an `int`, `Slider` a `float`.
        PropType::Number if kind == "stepper" => Ty::Int,
        PropType::Number => Ty::Float,
        PropType::String | PropType::Enum | PropType::Token => Ty::Str,
    };
    (ty, def.writable == Some(true))
}

pub struct Property {
    pub name: String,
    pub ty: Ty,
}

#[derive(Default)]
pub struct Data {
    /// Weft binding path (`$.user.email`) → its property, in document order.
    pub props: IndexMap<String, Property>,
}

#[derive(Default)]
struct Readers {
    writable: Vec<Ty>,
    read: Vec<Ty>,
}

fn collect(node: &Node, catalog: &Catalog, found: &mut IndexMap<String, Readers>) {
    for (name, value) in &node.props {
        if let Value::Bind { bind, .. } = value {
            let (ty, writable) = prop_type(catalog, &node.kind, name);
            let readers = found.entry(bind.clone()).or_default();
            if writable {
                readers.writable.push(ty);
            }
            readers.read.push(ty);
        }
    }
    let slots = node.slots.values().flatten();
    for child in node.children.iter().chain(slots) {
        if let Child::Node(child) = child {
            collect(child, catalog, found);
        }
    }
}

impl Data {
    pub fn infer(root: &Node, catalog: &Catalog, problems: &mut Vec<Unsupported>) -> Data {
        let mut found = IndexMap::new();
        collect(root, catalog, &mut found);
        let mut data = Data::default();
        let mut keys: IndexMap<String, String> = IndexMap::new();
        for (path, readers) in found {
            let Some(segments) = path.strip_prefix("$.") else {
                problems.push(Unsupported::new(
                    &path,
                    "loop variables need <each>, which the Slint target does not map yet (T67.2)",
                ));
                continue;
            };
            let parts: Vec<&str> = segments.split('.').collect();
            if parts.iter().any(|p| identifier(p).is_none()) {
                problems.push(Unsupported::new(
                    &path,
                    "a segment is an array index or no Slint identifier; iterate with <each> instead",
                ));
                continue;
            }
            let name = parts.join("-");
            let key = clash_key(&name);
            if TAKEN.contains(&key.as_str()) || key.starts_with("accessible-") {
                problems.push(Unsupported::new(
                    &path,
                    format!("`{name}` is already a property of a Slint Window"),
                ));
                continue;
            }
            if let Some(other) = keys.insert(key, path.clone()) {
                problems.push(Unsupported::new(
                    &path,
                    format!("its Slint property `{name}` clashes with the one of {other}"),
                ));
                continue;
            }
            let mut writable = readers.writable.clone();
            writable.dedup();
            if writable.len() > 1 {
                problems.push(Unsupported::new(
                    &path,
                    "inputs of different types write this path",
                ));
                continue;
            }
            let ty = writable.first().copied().unwrap_or_else(|| {
                [Ty::Str, Ty::Float, Ty::Int]
                    .into_iter()
                    .find(|t| readers.read.contains(t))
                    .unwrap_or(Ty::Bool)
            });
            data.props.insert(path, Property { name, ty });
        }
        data
    }

    /// The expression that reads `bind` (negated when `not`) as a value of type `want`; `None`
    /// when Slint has no such reading. A boolean read of text or a number is its truthiness
    /// (SPEC §2.1).
    pub fn read(&self, bind: &str, not: bool, want: Ty) -> Option<String> {
        let p = self.props.get(bind)?;
        let r = format!("root.{}", p.name);
        let ne = if not { "==" } else { "!=" };
        Some(match (want, p.ty, not) {
            (Ty::Bool, Ty::Bool, false) => r,
            (Ty::Bool, Ty::Bool, true) => format!("!{r}"),
            (Ty::Bool, Ty::Str, _) => format!("{r} {ne} \"\""),
            (Ty::Bool, Ty::Float | Ty::Int, _) => format!("{r} {ne} 0"),
            (_, _, true) => return None,
            // Slint converts numbers to text implicitly, and int and float into each other.
            (Ty::Str, _, false) | (Ty::Float | Ty::Int, Ty::Float | Ty::Int, false) => r,
            _ => return None,
        })
    }

    /// The `<=>` target for a writable prop of type `want`, when the binding can be written.
    pub fn two_way(&self, bind: &str, not: bool, want: Ty) -> Option<String> {
        let p = self.props.get(bind)?;
        (!not && p.ty == want).then(|| format!("root.{}", p.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weft_core::{Mode, ParseOptions, parse};

    fn data(markup: &str) -> (Data, Vec<Unsupported>) {
        let catalog = weft_catalog::core_catalog().unwrap();
        let options = ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            tokens: None,
            actions: None,
        };
        let doc = parse(markup, &options).document.unwrap();
        let mut problems = vec![];
        (Data::infer(&doc.root, &catalog, &mut problems), problems)
    }

    #[test]
    fn a_path_takes_the_type_of_the_input_that_writes_it() {
        let (d, p) = data(
            r#"<screen id="s" weft="0.1"><field id="e" label="E" value="{$.user.email}"/><button id="b" disabled="{!$.user.email}">Go</button></screen>"#,
        );
        assert!(p.is_empty(), "{p:?}");
        let prop = &d.props["$.user.email"];
        assert_eq!((prop.name.as_str(), prop.ty), ("user-email", Ty::Str));
        assert_eq!(
            d.read("$.user.email", true, Ty::Bool).unwrap(),
            r#"root.user-email == """#
        );
        assert_eq!(
            d.two_way("$.user.email", false, Ty::Str).unwrap(),
            "root.user-email"
        );
    }

    #[test]
    fn a_path_read_only_as_a_boolean_is_a_bool() {
        let (d, _) = data(
            r#"<screen id="s" weft="0.1"><button id="b" disabled="{$.busy}">Go</button></screen>"#,
        );
        assert_eq!(d.props["$.busy"].ty, Ty::Bool);
        assert_eq!(d.read("$.busy", true, Ty::Bool).unwrap(), "!root.busy");
    }

    #[test]
    fn paths_that_clash_in_slint_are_refused() {
        let (_, p) = data(
            r#"<screen id="s" weft="0.1"><text id="a" text="{$.a_b}"/><text id="b" text="{$.a.b}"/><text id="c" text="{$.title}"/></screen>"#,
        );
        let paths: Vec<&str> = p.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, ["$.a.b", "$.title"]);
    }
}
