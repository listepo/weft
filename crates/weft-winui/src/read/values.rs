//! Bindings, actions and token references of the XAML (SPEC §9, "From WinUI"): the `{x:Bind}`
//! forms the generator writes, with the `Weft` function bindings; `weft:On.*`, a native `Command`
//! and an event bound to the view model; and `Weft.*` resources.

use roxmltree::Node as Xml;
use weft_core::{Node, PropType, Value, is_action, is_binding, is_token, universal_prop};
use weft_import::LossKind;

use super::Reader;
use super::controls::Conv;
use crate::data::weft_name;

/// The `Weft` helper a function binding calls (`Weft.cs`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Function {
    Not,
    Truthy,
    Hide,
    /// `Weft.Link` and `Weft.Image`, which keep only the addresses SPEC §9 Trust allows.
    Address,
    /// `Weft.Earliest` and `Weft.Latest`: a date's `min` and `max`, open when not a date.
    Earliest,
    Latest,
}

/// An `{x:Bind}` value: a Weft data path, read through a helper or not.
struct Bound {
    path: String,
    function: Option<Function>,
    /// Without `Mode=OneWay` or `TwoWay`, x:Bind reads the value once.
    once: bool,
    /// `path` is the quoted text a function was called with, not a data path.
    literal: bool,
}

/// Reads `{x:Bind …}` on `el`, inside the `<each>` loops `loops` (outermost first); the error
/// says why it is not a Weft binding.
fn x_bind(el: Xml<'_, '_>, value: &str, loops: &[String]) -> Result<Bound, String> {
    let Some(inner) = value
        .strip_prefix("{x:Bind")
        .and_then(|v| v.strip_suffix('}'))
    else {
        return Err("only `{x:Bind}` names the page's view model".into());
    };
    let mut parts = inner.split(',');
    let first = parts.next().unwrap_or_default().trim();
    let expr = first.strip_prefix("Path=").unwrap_or(first).trim();
    let mut once = true;
    for option in parts {
        let (key, v) = option.split_once('=').unwrap_or((option, ""));
        match (key.trim(), v.trim()) {
            ("Mode", "OneWay" | "TwoWay") => once = false,
            ("Mode", "OneTime") | ("UpdateSourceTrigger", _) => {}
            (key, _) => return Err(format!("`{key}` changes what the binding reads")),
        }
    }
    let (function, path) = match expr.split_once('(') {
        None => (None, expr),
        Some((callee, rest)) => {
            let arg = rest.strip_suffix(')').ok_or("a call that does not end")?;
            // The prefix must name the page's own namespace, where `Weft.cs` puts the helpers.
            let helper = callee.trim().split_once(':').filter(|(prefix, _)| {
                el.lookup_namespace_uri(Some(prefix))
                    .is_some_and(|ns| ns.starts_with("using:"))
            });
            let function = match helper.map(|(_, name)| name) {
                Some("Weft.Not") => Function::Not,
                Some("Weft.Truthy") => Function::Truthy,
                Some("Weft.Hide") => Function::Hide,
                Some("Weft.Link" | "Weft.Image") => Function::Address,
                Some("Weft.Earliest") => Function::Earliest,
                Some("Weft.Latest") => Function::Latest,
                _ => return Err(format!("`{}` is not a `Weft` helper", callee.trim())),
            };
            (Some(function), arg.trim())
        }
    };
    // The generator writes a literal date bound as text, so a quote can only hold a date.
    if let Some(Function::Earliest | Function::Latest) = function
        && let Some(text) = path.strip_prefix('\'').and_then(|p| p.strip_suffix('\''))
    {
        return Ok(Bound {
            path: text.to_owned(),
            function,
            once,
            literal: true,
        });
    }
    Ok(Bound {
        path: data_path(path, loops)?,
        function,
        once,
        literal: false,
    })
}

/// `ViewModel.todos[0].title` → `$.todos.0.title`, each member its Weft name (`public_` →
/// `public`). Inside a template `WeftRoot` is the view model, a path of its own is the innermost
/// loop's item (`title` → `$todo.title`), and each `WeftParent.` steps out one loop. Outside the
/// loops an `<each>` declares, an item path has no Weft form.
fn data_path(p: &str, loops: &[String]) -> Result<String, String> {
    let not_ours = || format!("`{p}` is not a path of the page's view model");
    let (root, rest) = match p
        .strip_prefix("ViewModel.")
        .or_else(|| p.strip_prefix("WeftRoot."))
    {
        Some(rest) => ("$".to_owned(), rest),
        None => {
            let mut rest = p;
            let mut up = 0;
            while let Some(r) = rest.strip_prefix("WeftParent.") {
                rest = r;
                up += 1;
            }
            let at = loops.len().checked_sub(up + 1).ok_or_else(not_ours)?;
            (format!("${}", loops[at]), rest)
        }
    };
    // An index closes before a member, another index or the end: `a[0]b` is not a path.
    let closed = rest
        .split(']')
        .skip(1)
        .all(|after| after.is_empty() || after.starts_with(['.', '[']));
    let rest = rest.replace(']', "").replace('[', ".");
    let names: Vec<_> = rest.split('.').map(weft_name).collect();
    let path = format!("{root}.{}", names.join("."));
    if closed && is_binding(&path) {
        Ok(path)
    } else {
        Err(format!("`{p}` is not a Weft data path"))
    }
}

/// `ViewModel.todo_addCommand` → `todo.add`: action names hold no `_` (SPEC §2.2). A `Command`
/// property names a command; an event names a method of the view model.
fn action(bound: &Bound, command: bool) -> Option<String> {
    let name = bound.path.strip_prefix("$.")?;
    let name = if command {
        name.strip_suffix("Command")?
    } else {
        name
    };
    let action = name.replace('_', ".");
    (bound.function.is_none() && is_action(&action)).then_some(action)
}

/// `{StaticResource Weft.space.md}` → `space.md`; a padding's resource is the token's
/// `Thickness`, keyed with `.thickness` after the path.
fn resource(value: &str, thickness: bool) -> Option<&str> {
    let inner = value
        .strip_prefix("{StaticResource ")
        .or_else(|| value.strip_prefix("{ThemeResource "))?
        .strip_suffix('}')?;
    let path = inner.trim().strip_prefix("Weft.")?;
    let path = match thickness {
        true => path.strip_suffix(".thickness")?,
        false => path,
    };
    is_token(path).then_some(path)
}

impl Reader<'_> {
    /// The list an `ItemsSource` binding repeats, as a plain Weft path.
    pub(super) fn list_path(&self, el: Xml<'_, '_>, value: &str) -> Option<String> {
        let bound = x_bind(el, value, &self.loops).ok()?;
        (bound.function.is_none() && !bound.once).then_some(bound.path)
    }

    /// `value` as `prop` of `node`, when the kind has the prop and the prop takes it.
    fn put(&mut self, node: &mut Node, prop: &str, value: Value, here: &str, attr: &str) {
        let def = self
            .catalog
            .components
            .get(&node.kind)
            .and_then(|c| c.prop(prop))
            .or_else(|| universal_prop(prop));
        let token = |t: &str| self.tokens.get(t).map(|t| &t.kind);
        let problem = match (&value, def) {
            (_, None) => Some(format!("a `{}` has no `{prop}`", node.kind)),
            (Value::Bind { .. }, Some(d)) if d.bindable == Some(false) => {
                Some(format!("`{prop}` cannot be bound"))
            }
            // SPEC §5.1: only a boolean is negated.
            (Value::Bind { not: true, .. }, Some(d)) if d.kind != PropType::Boolean => {
                Some(format!("`{prop}` takes no negated binding"))
            }
            (Value::Token(t), Some(d)) => match token(t) {
                None => Some(format!("`{t}` is not a token of the project")),
                Some(kind) if d.kind != PropType::Token || d.token_type.as_ref() != Some(kind) => {
                    Some(format!("the {kind} token `{t}` does not fit `{prop}`"))
                }
                Some(_) => None,
            },
            _ => None,
        };
        let loss = match value {
            Value::Token(_) => LossKind::Tokens,
            _ => LossKind::Bindings,
        };
        if let Some(problem) = problem {
            return self.lose(loss, here, format!("`{attr}`: {problem}"));
        }
        match node.props.get(prop) {
            Some(old) if *old != value => {
                let note = format!("`{attr}` disagrees with the `{prop}` already read");
                self.lose(loss, here, note);
            }
            _ => {
                node.props.insert(prop.to_owned(), value);
            }
        }
    }

    /// An `{x:Bind}` value of `attr`, read as `prop` with the attribute's conversion undone; an
    /// empty prop is the element's text, which is its `text` prop when bound.
    pub(super) fn bound(
        &mut self,
        el: Xml<'_, '_>,
        node: &mut Node,
        (attr, prop, conv): (&str, &str, &Conv),
        value: &str,
        here: &str,
    ) {
        let tag = el.tag_name().name();
        let bound = match x_bind(el, value, &self.loops) {
            Ok(b) => b,
            Err(why) => {
                return self.lose(
                    LossKind::Bindings,
                    here,
                    format!("`{attr}` on `{tag}`: {why}"),
                );
            }
        };
        use Function::{Address, Earliest, Hide, Latest, Not, Truthy};
        // `IsEnabled` and `Visibility` say the opposite of `disabled` and `hidden`.
        let not = match (conv, bound.function) {
            (Conv::Same, None | Some(Truthy)) | (Conv::Not, Some(Not)) => false,
            (Conv::Same, Some(Address)) if matches!(attr, "NavigateUri" | "Source") => false,
            (Conv::Same, Some(Earliest)) if attr == "MinDate" => false,
            (Conv::Same, Some(Latest)) if attr == "MaxDate" => false,
            (Conv::Same, Some(Not)) | (Conv::Not, None | Some(Truthy)) => true,
            (Conv::Words(_), None) if prop == "hidden" => true,
            (Conv::Words(_), Some(Hide)) if prop == "hidden" => false,
            _ => {
                let note = format!("`{attr}` on `{tag}`: a binding with no Weft form");
                return self.lose(LossKind::Bindings, here, note);
            }
        };
        if bound.literal {
            return self.set(node, prop, &bound.path, here, attr);
        }
        if bound.once {
            let note = format!(
                "`{attr}` on `{tag}`: without `Mode=OneWay` x:Bind reads the value once; it is read as a live binding"
            );
            self.lose(LossKind::Bindings, here, note);
        }
        let prop = if prop.is_empty() { "text" } else { prop };
        let value = Value::Bind {
            bind: bound.path,
            not,
        };
        self.put(node, prop, value, here, attr);
    }

    /// An action bound to `attr`: the first of `events` the kind has. A `weft:On.Submit` on a
    /// button is the enclosing form's submit, so it is the button's `submit`.
    pub(super) fn event(
        &mut self,
        el: Xml<'_, '_>,
        node: &mut Node,
        (attr, events): (&str, &[&str]),
        value: &str,
        here: &str,
    ) {
        let tag = el.tag_name().name();
        if node.kind == "button" && events == ["submit"] {
            return self.put(node, "submit", Value::Bool(true), here, attr);
        }
        let command = attr == "Command" || attr.starts_with("On.");
        let bound = x_bind(el, value, &self.loops).ok();
        let Some(action) = bound.and_then(|b| action(&b, command)) else {
            let note = format!(
                "`{attr}` on `{tag}`: not a command of the view model named after a Weft action"
            );
            return self.lose(LossKind::Actions, here, note);
        };
        let known = self
            .catalog
            .components
            .get(&node.kind)
            .and_then(|c| c.events.as_ref());
        let Some(event) = events
            .iter()
            .find(|e| known.is_some_and(|k| k.iter().any(|x| x == *e)))
        else {
            let note = format!("`{attr}` on `{tag}`: a `{}` has no such event", node.kind);
            return self.lose(LossKind::Actions, here, note);
        };
        match node.on.get(*event) {
            Some(old) if *old != action => {
                let note = format!("`{attr}` on `{tag}`: `on-{event}` already calls `{old}`");
                self.lose(LossKind::Actions, here, note);
            }
            _ => {
                node.on.insert((*event).to_owned(), action);
            }
        }
    }

    /// A `Weft.*` resource as the token prop `prop`; false when `value` is not one.
    pub(super) fn token(
        &mut self,
        node: &mut Node,
        (attr, prop): (&str, &str),
        value: &str,
        here: &str,
    ) -> bool {
        let Some(path) = resource(value, attr == "Padding") else {
            return false;
        };
        self.put(node, prop, Value::Token(path.to_owned()), here, attr);
        true
    }
}
