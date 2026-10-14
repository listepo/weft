//! Expression and modifier helpers of the SwiftUI reader.

use weft_core::Value;

use super::super::syntax::{Arg, Call, Closure, Expr, Stmt};
use crate::data::Leaf;
use crate::swift;

use super::types::VIEW_MEMBERS;

pub use weft_import::element_path;

/// `[A-Za-z][A-Za-z0-9_-]*` (SPEC §2.2).
pub fn is_id(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*` (SPEC §2.2).
pub(super) fn is_action(s: &str) -> bool {
    s.split('.').all(|seg| {
        let mut chars = seg.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_alphanumeric())
    })
}

pub(super) fn is_path_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A modifier applied to a view: `.name(args) { closures }`.
#[derive(Clone)]
pub(super) struct Mod {
    pub(super) name: String,
    pub(super) args: Vec<Arg>,
    pub(super) closures: Vec<Closure>,
}

pub(super) struct Mods(pub(super) Vec<Mod>);

impl Mods {
    pub(super) fn take(&mut self, name: &str) -> Option<Mod> {
        let i = self.0.iter().position(|m| m.name == name)?;
        Some(self.0.remove(i))
    }

    pub(super) fn take_all(&mut self, name: &str) -> Vec<Mod> {
        let (taken, kept) = std::mem::take(&mut self.0)
            .into_iter()
            .partition(|m| m.name == name);
        self.0 = kept;
        taken
    }

    pub(super) fn has(&self, name: &str) -> bool {
        self.0.iter().any(|m| m.name == name)
    }

    /// Takes `.name(.value)` when its first argument is that implicit member.
    pub(super) fn take_flag(&mut self, name: &str, value: &str) -> bool {
        let i = self.0.iter().position(|m| {
            m.name == name
                && matches!(m.args.first().map(|a| &a.value), Some(Expr::Implicit(v)) if v == value)
        });
        match i {
            Some(i) => {
                self.0.remove(i);
                true
            }
            None => false,
        }
    }
}

pub(super) fn first_arg(args: &[Arg]) -> Option<&Expr> {
    args.iter().find(|a| a.label.is_none()).map(|a| &a.value)
}

pub(super) fn arg<'e>(args: &'e [Arg], label: &str) -> Option<&'e Expr> {
    args.iter()
        .find(|a| a.label.as_deref() == Some(label))
        .map(|a| &a.value)
}

pub(super) fn closure<'c>(closures: &'c [Closure], label: Option<&str>) -> Option<&'c Closure> {
    closures.iter().find(|c| c.label.as_deref() == label)
}

pub(super) fn implicit(e: Option<&Expr>) -> Option<&str> {
    match e {
        Some(Expr::Implicit(name)) => Some(name),
        _ => None,
    }
}

pub(super) fn ident(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident { name, .. } => Some(name),
        _ => None,
    }
}

/// `Color.clear` → "Color.clear"; `Text` → "Text".
pub(super) fn type_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::Member { base, name, .. } => Some(format!("{}.{name}", type_name(base)?)),
        _ => None,
    }
}

/// Splits `base.m1().m2()` into the base view and its modifiers, innermost first.
pub(super) fn unwind(expr: &Expr) -> (&Expr, Vec<Mod>) {
    let mut mods = vec![];
    let mut e = expr;
    while let Expr::Call(Call {
        callee,
        args,
        closures,
    }) = e
    {
        let Expr::Member { base, name, .. } = callee.as_ref() else {
            break;
        };
        // `Color.clear` and `Image.init` are not modifiers: their base is a type.
        if matches!(base.as_ref(), Expr::Ident { name: n, .. } if n.starts_with(|c: char| c.is_ascii_uppercase()))
        {
            break;
        }
        mods.push(Mod {
            name: name.clone(),
            args: args.clone(),
            closures: closures.clone(),
        });
        e = base;
    }
    mods.reverse();
    (e, mods)
}

/// `Spacer()` or `Spacer(minLength: e)` with no modifier: `Some(None)` or `Some(Some(e))`.
pub(super) fn spacer(s: &Stmt) -> Option<Option<&Expr>> {
    let Stmt::Expr(Expr::Call(c)) = s else {
        return None;
    };
    if ident(&c.callee) != Some("Spacer") || !c.closures.is_empty() {
        return None;
    }
    match c.args.as_slice() {
        [] => Some(None),
        [a] if a.label.as_deref() == Some("minLength") => Some(Some(&a.value)),
        _ => None,
    }
}

/// Removes the spacers of a `space-between` row, also from the bodies of its `ForEach`es, where
/// the first item's spacer is behind an `if`, collecting their `minLength`s.
pub(super) fn strip_spacers(stmts: &mut Vec<Stmt>, in_each: bool, found: &mut Vec<Option<Expr>>) {
    stmts.retain(|s| {
        let m = match s {
            Stmt::If {
                condition: Expr::Binary { .. },
                then,
                otherwise,
            } if in_each && otherwise.is_empty() => match then.as_slice() {
                [inner] => spacer(inner),
                _ => None,
            },
            other => spacer(other),
        };
        match m {
            Some(m) => {
                found.push(m.cloned());
                false
            }
            None => true,
        }
    });
    for s in stmts.iter_mut() {
        if let Stmt::Expr(e) = s
            && let Some(each) = for_each_mut(e)
            && let Some(c) = each.closures.first_mut()
        {
            strip_spacers(&mut c.body, true, found);
        }
    }
}

/// The `ForEach` call under a chain of modifiers.
pub(super) fn for_each_mut(e: &mut Expr) -> Option<&mut Call> {
    let Expr::Call(c) = e else {
        return None;
    };
    if ident(&c.callee) == Some("ForEach") {
        return Some(c);
    }
    match c.callee.as_mut() {
        Expr::Member { base, .. } => for_each_mut(base),
        _ => None,
    }
}

/// Takes `.frame(maxWidth: .infinity)` alone, which `generate` gives a growing row child.
pub(super) fn take_grow(mods: &mut Mods) -> bool {
    let i = mods.0.iter().position(|m| {
        m.name == "frame"
            && matches!(m.args.as_slice(), [a]
                if a.label.as_deref() == Some("maxWidth")
                    && implicit(Some(&a.value)) == Some("infinity"))
    });
    i.map(|i| mods.0.remove(i)).is_some()
}

pub(super) fn is_literal(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Str(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Array(_) | Expr::Neg(_)
    )
}
pub(super) fn name_is(call: &Call, name: &str) -> bool {
    ident(&call.callee) == Some(name)
}

/// A literal read where a prop wants another type, converted as Weft reads values (SPEC §2.1).
pub(super) fn convert(v: Value, leaf: Leaf) -> Value {
    match (leaf, v) {
        (Leaf::Text, Value::Number(n)) => Value::String(swift::number_literal(n)),
        (Leaf::Text, Value::Bool(b)) => Value::String(b.to_string()),
        (_, v) => v,
    }
}

/// The Weft name of a loop variable: `generate` adds `_` to names Swift reserves.
pub(super) fn loop_name(ident: &str) -> String {
    let stripped = ident.strip_suffix('_').unwrap_or(ident);
    let base =
        if stripped != ident && (swift::is_keyword(stripped) || VIEW_MEMBERS.contains(&stripped)) {
            stripped
        } else {
            ident
        };
    let mut out = String::new();
    for c in base.chars().filter(char::is_ascii_alphanumeric) {
        if out.is_empty() {
            if c.is_ascii_alphabetic() {
                out.push(c.to_ascii_lowercase());
            }
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        "item".to_owned()
    } else {
        out
    }
}

/// A Weft action name from a Swift function name.
pub(super) fn action_name(name: &str) -> String {
    let n = loop_name(name);
    if n == "item" && !name.starts_with("item") {
        "action".to_owned()
    } else {
        n
    }
}
