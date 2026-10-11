//! Form controls: sliders, steppers, pickers and toggles.
use weft_core::{Child, Node, Value};

use super::super::syntax::{Call, Expr, Stmt};
use crate::data::Leaf;

use super::helpers::{Mods, arg, closure, first_arg, ident, implicit, name_is};
use super::types::{Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn placeholder(&mut self, node: &mut Node, call: &Call, path: &str) {
        if let Some(Expr::Call(p)) = arg(&call.args, "prompt")
            && let Some(e) = first_arg(&p.args)
            && let Some(v) = self.value(e, Leaf::Text, path)
        {
            node.props.insert("placeholder".to_owned(), v);
        }
    }

    /// A number prop read from `e`; `nil` and anything else that is not a number is absent.
    pub(super) fn number(&mut self, node: &mut Node, prop: &str, e: Option<&Expr>, path: &str) {
        if let Some(v) = e.and_then(|e| self.value(e, Leaf::Double, path)) {
            node.props.insert(prop.to_owned(), v);
        }
    }

    /// The two ends of `lo...hi`, or of the helper that keeps a range from being reversed.
    pub(super) fn range(e: Option<&Expr>) -> (Option<&Expr>, Option<&Expr>) {
        match e {
            Some(Expr::Binary { left, op, right }) if op == "..." => {
                (Some(left.as_ref()), Some(right.as_ref()))
            }
            Some(Expr::Call(c)) if matches!(ident(&c.callee), Some("weftRange" | "weftBounds")) => {
                let at = |i: usize| c.args.get(i).map(|a| &a.value);
                (at(0), at(1))
            }
            _ => (None, None),
        }
    }

    pub(super) fn slider(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("slider", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "value"),
            Leaf::Double,
            &here,
        );
        let (lo, hi) = Self::range(arg(&call.args, "in"));
        self.number(&mut node, "min", lo, &here);
        self.number(&mut node, "max", hi, &here);
        self.number(&mut node, "step", arg(&call.args, "step"), &here);
        self.finish(node, mods, &here)
    }

    pub(super) fn stepper(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("stepper", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "value"),
            Leaf::Double,
            &here,
        );
        let (lo, hi) = Self::range(arg(&call.args, "in"));
        self.number(&mut node, "min", lo, &here);
        self.number(&mut node, "max", hi, &here);
        self.number(&mut node, "step", arg(&call.args, "step"), &here);
        self.finish(node, mods, &here)
    }

    pub(super) fn date_picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("date-picker", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        mods.take("weftUTC");
        // `weftDate(binding, type)` is what `generate` prints; a plain `Date` binding is not data.
        let selection = arg(&call.args, "selection");
        let (binding, kind) = match selection {
            // The helper leaves an absent `type` out, so the second argument is the `type` prop.
            Some(Expr::Call(c)) if ident(&c.callee) == Some("weftDate") => (
                c.args.first().map(|a| &a.value),
                c.args
                    .get(1)
                    .and_then(|a| self.value(&a.value, Leaf::Text, &here)),
            ),
            other => {
                let components = Self::components(arg(&call.args, "displayedComponents"));
                (
                    other,
                    components.filter(|k| *k != Value::String("date".to_owned())),
                )
            }
        };
        self.writes(&mut node, "value", binding, Leaf::Text, &here);
        if let Some(kind) = kind {
            node.props.insert("type".to_owned(), kind);
        }
        if let Some(Expr::Call(c)) = arg(&call.args, "in")
            && ident(&c.callee) == Some("weftDates")
        {
            for (prop, i) in [("min", 0), ("max", 1)] {
                if let Some(v) = c
                    .args
                    .get(i)
                    .and_then(|a| self.value(&a.value, Leaf::Text, &here))
                    && v != Value::String(String::new())
                {
                    node.props.insert(prop.to_owned(), v);
                }
            }
        }
        self.finish(node, mods, &here)
    }

    /// The `type` a hand-written `displayedComponents: [.date, .hourAndMinute]` stands for.
    pub(super) fn components(e: Option<&Expr>) -> Option<Value> {
        let names: Vec<&str> = match e? {
            Expr::Array(items) => items.iter().filter_map(|i| implicit(Some(i))).collect(),
            other => implicit(Some(other)).into_iter().collect(),
        };
        let kind = match (names.contains(&"date"), names.contains(&"hourAndMinute")) {
            (true, true) => "datetime",
            (false, true) => "time",
            _ => "date",
        };
        Some(Value::String(kind.to_owned()))
    }

    pub(super) fn color_picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("color-picker", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        let binding = match arg(&call.args, "selection") {
            Some(Expr::Call(c)) if ident(&c.callee) == Some("weftColor") => {
                c.args.first().map(|a| &a.value)
            }
            other => other,
        };
        self.writes(&mut node, "value", binding, Leaf::Text, &here);
        self.finish(node, mods, &here)
    }

    /// The text field and the menu of options that `generate` prints for a combobox.
    pub(super) fn combobox(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        mods.take("weftCombobox");
        let Some((mut node, here)) = self.element("combobox", &mut mods, "", path) else {
            return vec![];
        };
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let calls: Vec<&Call> = body
            .iter()
            .filter_map(|s| match s {
                Stmt::Expr(Expr::Call(c)) => Some(c),
                _ => None,
            })
            .collect();
        if let Some(field) = calls.iter().find(|c| name_is(c, "TextField")) {
            self.caption(&mut node, field, &here);
            self.writes(
                &mut node,
                "value",
                arg(&field.args, "text"),
                Leaf::Text,
                &here,
            );
            self.placeholder(&mut node, field, &here);
        }
        let options = calls
            .iter()
            .find(|c| name_is(c, "Menu"))
            .and_then(|m| closure(&m.closures, None))
            .and_then(|c| {
                c.body.iter().find_map(|s| match s {
                    Stmt::Expr(Expr::Call(p)) if name_is(p, "Picker") => {
                        closure(&p.closures, None).map(|c| c.body.clone())
                    }
                    _ => None,
                })
            })
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&options, Place::Choice("option"), &here));
        self.finish(node, mods, &here)
    }

    pub(super) fn toggle(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let checkbox = mods.0.iter().any(|m| {
            m.name == "toggleStyle"
                && match first_arg(&m.args) {
                    Some(Expr::Implicit(s)) => s == "checkbox",
                    Some(Expr::Call(c)) => ident(&c.callee) == Some("WeftCheckboxStyle"),
                    _ => false,
                }
        });
        mods.take("toggleStyle");
        let kind = if checkbox { "checkbox" } else { "switch" };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "checked",
            arg(&call.args, "isOn"),
            Leaf::Bool,
            &here,
        );
        self.finish(node, mods, &here)
    }

    pub(super) fn picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let style = mods
            .take("pickerStyle")
            .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
        let (kind, child) = match style.as_deref() {
            Some("segmented") => ("segmented-control", "segment"),
            Some("inline" | "radioGroup") => ("radio-group", "radio"),
            _ => ("select", "option"),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "selection"),
            Leaf::Text,
            &here,
        );
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Choice(child), &here));
        self.finish(node, mods, &here)
    }
}
