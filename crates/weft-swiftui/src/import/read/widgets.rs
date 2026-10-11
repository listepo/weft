//! Text, images and the widgets that own a caption and a form field.
use weft_core::{Child, Node, Value};

use super::super::LossKind;
use super::super::syntax::{Call, Expr, Stmt, plain_string};
use crate::data::Leaf;

use super::helpers::{Mods, arg, closure, first_arg, ident, implicit, name_is, unwind};
use super::types::{Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn text_content(&mut self, node: &mut Node, call: &Call, path: &str) {
        if let Some(e) = arg(&call.args, "verbatim") {
            if let Some(v) = self.value(e, Leaf::Text, path) {
                node.props.insert("text".to_owned(), v);
            }
            return;
        }
        let first = first_arg(&call.args).or_else(|| arg(&call.args, "title"));
        match first {
            Some(Expr::Str(parts)) => match plain_string(parts) {
                Some(s) if s.is_empty() => {}
                Some(s) => {
                    let s = self.string(&s, path);
                    node.children.push(Child::Text(s));
                }
                None => match self.interpolated(parts, Leaf::Text, path) {
                    Some(v @ Value::Bind { .. }) => {
                        node.props.insert("text".to_owned(), v);
                    }
                    Some(Value::String(s)) if !s.is_empty() => node.children.push(Child::Text(s)),
                    _ => {}
                },
            },
            Some(e) => match self.value(e, Leaf::Text, path) {
                Some(v) => {
                    node.props.insert("text".to_owned(), v);
                }
                None => self.lose(
                    LossKind::Text,
                    path,
                    "text that is not a literal or a data path is not kept",
                ),
            },
            None => {
                // `Button { } label: { Text("…") }` and `Label { Text("…") } icon: { }`.
                let label = closure(&call.closures, Some("label"))
                    .or_else(|| {
                        let only = (call.closures.len() == 1)
                            .then(|| call.closures.first())
                            .flatten()?;
                        let action = call
                            .args
                            .iter()
                            .any(|a| a.label.as_deref() == Some("action"));
                        (only.label.is_none() && action).then_some(only)
                    })
                    .or_else(|| closure(&call.closures, Some("title")));
                if let Some(label) = label {
                    let mut texts = vec![];
                    for s in &label.body {
                        let Stmt::Expr(e) = s else { continue };
                        let (base, mods) = unwind(e);
                        if let Expr::Call(c) = base
                            && matches!(ident(&c.callee), Some("Text" | "Label"))
                        {
                            texts.push(c.clone());
                            if !mods.is_empty() {
                                self.lose(
                                    LossKind::Layout,
                                    path,
                                    "modifiers of a label's text are not kept",
                                );
                            }
                        } else {
                            self.lose(
                                LossKind::Text,
                                path,
                                "a label view that is not text is not kept",
                            );
                        }
                    }
                    for c in texts {
                        self.text_content(node, &c, path);
                    }
                }
            }
        }
    }

    pub(super) fn text_kind(
        &mut self,
        kind: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let name = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, &name, path) else {
            return vec![];
        };
        self.text_content(&mut node, call, &here);
        if ident(&call.callee) == Some("Label") && arg(&call.args, "systemImage").is_some() {
            self.lose(LossKind::Props, &here, "the label's icon is not kept");
        }
        match kind {
            "heading" => {
                let mut level = None;
                if let Some(m) = mods.take("accessibilityHeading") {
                    level = match first_arg(&m.args) {
                        Some(Expr::Implicit(h)) => h
                            .strip_prefix('h')
                            .and_then(|n| n.parse::<u8>().ok())
                            .filter(|n| (1..=6).contains(n))
                            .map(|n| Value::Number(f64::from(n))),
                        Some(Expr::Call(c)) if ident(&c.callee) == Some("weftHeadingLevel") => {
                            first_arg(&c.args).and_then(|e| self.value(e, Leaf::Int, &here))
                        }
                        _ => None,
                    };
                }
                if let Some(m) = mods.take("font") {
                    let from_font = match implicit(first_arg(&m.args)) {
                        Some("largeTitle") => Some(1.0),
                        Some("title") => Some(2.0),
                        Some("title2") => Some(3.0),
                        Some("title3") => Some(4.0),
                        Some("headline") => Some(5.0),
                        Some("subheadline") => Some(6.0),
                        _ => None,
                    };
                    if level.is_none() {
                        level = from_font.map(Value::Number);
                    }
                }
                mods.take_flag("accessibilityAddTraits", "isHeader");
                match level {
                    Some(l) => {
                        node.props.insert("level".to_owned(), l);
                    }
                    None => {
                        self.lose(
                            LossKind::Values,
                            &here,
                            "the heading level is not read; level 2 stands in",
                        );
                        node.props.insert("level".to_owned(), Value::Number(2.0));
                    }
                }
            }
            "text" => {
                let style = mods
                    .take("foregroundStyle")
                    .or_else(|| mods.take("foregroundColor"));
                if let Some(m) = style {
                    let tone = match implicit(first_arg(&m.args)) {
                        Some("primary") => Some("default"),
                        Some("secondary") => Some("muted"),
                        Some("green") => Some("success"),
                        Some("orange" | "yellow") => Some("warning"),
                        Some("red") => Some("danger"),
                        _ => None,
                    };
                    match tone {
                        Some(t) => {
                            node.props
                                .insert("tone".to_owned(), Value::String(t.to_owned()));
                        }
                        None => self.lose(
                            LossKind::Layout,
                            &here,
                            "a text color with no Weft tone is not kept",
                        ),
                    }
                }
            }
            "column" => {
                mods.take_flag("font", "headline");
            }
            "radio" | "option" | "segment" => {
                match mods.take("tag").and_then(|m| first_arg(&m.args).cloned()) {
                    Some(e) => {
                        if let Some(v) = self.value(&e, Leaf::Text, &here) {
                            node.props.insert("value".to_owned(), v);
                        }
                    }
                    None => self.lose(
                        LossKind::Values,
                        &here,
                        "a choice without `.tag` gets its text as value",
                    ),
                }
            }
            _ => {}
        }
        self.finish(node, mods, &here)
    }

    pub(super) fn image(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some((mut node, here)) = self.element("image", &mut mods, "", path) else {
            return vec![];
        };
        let src = match name {
            "AsyncImage" => arg(&call.args, "url").and_then(|e| match e {
                Expr::Call(c) if ident(&c.callee) == Some("URL") => arg(&c.args, "string").cloned(),
                other => Some(other.clone()),
            }),
            _ => first_arg(&call.args)
                .or_else(|| arg(&call.args, "systemName"))
                .cloned(),
        };
        match src.and_then(|e| self.value(&e, Leaf::Text, &here)) {
            Some(v) => {
                if name == "Image" {
                    self.lose(
                        LossKind::Values,
                        &here,
                        "an asset or symbol name is kept as the image source",
                    );
                }
                node.props.insert("src".to_owned(), v);
            }
            None => self.lose(LossKind::Values, &here, "the image source is not read"),
        }
        self.finish(node, mods, &here)
    }

    /// The `WeftModel` helper the generator prints; `src` comes back from its `weftProp` marker.
    pub(super) fn model(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("model", &mut mods, "", path) else {
            return vec![];
        };
        for (arg_name, prop) in [("usdz", "usdz"), ("fallback", "fallback")] {
            if let Some(v) =
                arg(&call.args, arg_name).and_then(|e| self.value(e, Leaf::Text, &here))
            {
                node.props.insert(prop.to_owned(), v);
            }
        }
        self.finish(node, mods, &here)
    }

    pub(super) fn button(
        &mut self,
        kind: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let title = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, &title, path) else {
            return vec![];
        };
        self.text_content(&mut node, call, &here);
        // The action: the unlabeled trailing closure, or `action:`, unless that closure is the label.
        let action = match (first_arg(&call.args), arg(&call.args, "action")) {
            (_, Some(Expr::Closure(c))) => Some(c.body.clone()),
            (Some(_), None) => closure(&call.closures, None).map(|c| c.body.clone()),
            (None, None) if closure(&call.closures, Some("label")).is_some() => {
                closure(&call.closures, None).map(|c| c.body.clone())
            }
            _ => None,
        };
        if let Some(body) = action {
            let h = self.event(&mut node, "press", &body, &here);
            if h.submit.is_some() && self.declares_prop(kind, "submit") {
                node.props.insert("submit".to_owned(), Value::Bool(true));
            }
            if let Some(href) = h.href
                && let Some(v) = self.value(&href, Leaf::Text, &here)
            {
                node.props.insert("href".to_owned(), v);
            }
        }
        if name_is(call, "Link")
            && let Some(dest) = arg(&call.args, "destination")
        {
            let dest = match dest {
                Expr::Call(c) if ident(&c.callee) == Some("URL") => arg(&c.args, "string").cloned(),
                other => Some(other.clone()),
            };
            if let Some(v) = dest.and_then(|e| self.value(&e, Leaf::Text, &here)) {
                node.props.insert("href".to_owned(), v);
            }
        }
        if kind == "button" {
            let danger = matches!(implicit(arg(&call.args, "role")), Some("destructive"));
            let style = mods
                .take("buttonStyle")
                .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
            let variant = match (danger, style.as_deref()) {
                (true, _) => Some("danger"),
                (false, Some("borderedProminent")) => Some("primary"),
                (false, Some("bordered")) => Some("secondary"),
                _ => None,
            };
            if let Some(v) = variant {
                node.props
                    .insert("variant".to_owned(), Value::String(v.to_owned()));
            }
        } else if kind == "link" {
            mods.take_flag("buttonStyle", "borderless");
        }
        self.finish(node, mods, &here)
    }

    pub(super) fn form(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("form", &mut mods, "", path) else {
            return vec![];
        };
        let mut body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let mut footer = None;
        // `Form { Section { … } footer: { … } }` is a form with a footer slot.
        if let [Stmt::Expr(e)] = body.as_slice()
            && let Expr::Call(c) = e
            && ident(&c.callee) == Some("Section")
            && c.args.is_empty()
            && let Some(f) = closure(&c.closures, Some("footer"))
            && closure(&c.closures, Some("header")).is_none()
        {
            footer = Some(f.body.clone());
            body = closure(&c.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default();
        }
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        if let Some(f) = footer {
            let views = self.nested(|r| r.views(&f, Place::Nodes, &format!("{here}/slot[footer]")));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert("footer".to_owned(), views);
            }
        }
        self.finish(node, mods, &here)
    }

    /// The caption of a control: its first argument or `label:` closure.
    pub(super) fn caption(&mut self, node: &mut Node, call: &Call, path: &str) {
        let label = first_arg(&call.args).cloned().or_else(|| {
            let c = closure(&call.closures, Some("label")).or_else(|| {
                (call.closures.len() == 1)
                    .then(|| call.closures.first())
                    .flatten()
            })?;
            match c.body.as_slice() {
                [Stmt::Expr(e)] => {
                    let (base, _) = unwind(e);
                    match base {
                        Expr::Call(t) if matches!(ident(&t.callee), Some("Text" | "Label")) => {
                            first_arg(&t.args).cloned()
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        });
        if let Some(e) = label
            && let Some(v) = self.value(&e, Leaf::Text, path)
        {
            node.props.insert("label".to_owned(), v);
        }
    }

    /// A control's binding: data it writes, or a constant that reads as a literal.
    pub(super) fn writes(
        &mut self,
        node: &mut Node,
        prop: &str,
        e: Option<&Expr>,
        leaf: Leaf,
        path: &str,
    ) {
        let Some(e) = e else { return };
        let v = match e {
            Expr::Call(c) if matches!(c.callee.as_ref(), Expr::Implicit(n) if n == "constant") => {
                match first_arg(&c.args).and_then(|a| self.value(a, leaf, path)) {
                    // A blank constant is what `generate` prints for an absent prop.
                    Some(Value::String(s)) if s.is_empty() => None,
                    Some(Value::Bool(false)) => None,
                    Some(Value::Number(n)) if n == 0.0 && leaf == Leaf::Double => None,
                    other => other,
                }
            }
            other => match self.bind(other, false) {
                Some(v) => Some(v),
                None => {
                    self.lose(
                        LossKind::Bindings,
                        path,
                        format!("the `{prop}` binding is not a data path"),
                    );
                    None
                }
            },
        };
        if let Some(v) = v {
            node.props.insert(prop.to_owned(), v);
        }
    }

    pub(super) fn field(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let title = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element("field", &mut mods, &title, path) else {
            return vec![];
        };
        if name != "TextEditor" {
            self.caption(&mut node, call, &here);
        }
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "text"),
            Leaf::Text,
            &here,
        );
        self.placeholder(&mut node, call, &here);
        let mut field_type = match name {
            "SecureField" => Some(Value::String("password".to_owned())),
            "TextEditor" => Some(Value::String("multiline".to_owned())),
            _ if implicit(arg(&call.args, "axis")) == Some("vertical") => {
                Some(Value::String("multiline".to_owned()))
            }
            _ => None,
        };
        if let Some(m) = mods.take("weftFieldType") {
            field_type = first_arg(&m.args).and_then(|e| self.value(e, Leaf::Text, &here));
        }
        for modifier in ["keyboardType", "textContentType"] {
            if let Some(m) = mods.take(modifier) {
                let t = match implicit(first_arg(&m.args)) {
                    Some("emailAddress") => Some("email"),
                    Some("numberPad" | "decimalPad" | "numbersAndPunctuation") => Some("number"),
                    Some("password" | "newPassword") => Some("password"),
                    _ => None,
                };
                if field_type.is_none()
                    && let Some(t) = t
                {
                    field_type = Some(Value::String(t.to_owned()));
                }
            }
        }
        if let Some(t) = field_type {
            node.props.insert("type".to_owned(), t);
        }
        mods.take("textInputAutocapitalization");
        mods.take("autocorrectionDisabled");
        self.finish(node, mods, &here)
    }
}
