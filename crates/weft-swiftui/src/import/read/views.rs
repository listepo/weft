//! Turning Swift view expressions into a Weft node tree.
use weft_core::{Child, Node, Value};

use super::super::syntax::{Arg, Call, Expr, Stmt, plain_string};
use super::super::{LossKind, MAX_DEPTH, MAX_NODES};
use crate::data::{Leaf, prop_leaf};

use super::helpers::{
    Mod, Mods, arg, closure, element_path, first_arg, ident, implicit, take_grow, type_name, unwind,
};
use super::types::{LAYOUT_MODIFIERS, Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn screen(&mut self, statements: &[Stmt]) -> Node {
        let mut root = Node::new("screen");
        // The generated root, or any single stack, is the screen itself.
        let single = match statements {
            [Stmt::Expr(e)] => Some(e),
            _ => None,
        };
        let mut e = single;
        while let Some(expr) = e {
            let (base, _) = unwind(expr);
            let name = match base {
                Expr::Call(c) => type_name(&c.callee),
                other => type_name(other),
            };
            match name.as_deref() {
                Some("NavigationStack" | "NavigationView" | "ScrollView") => {
                    let Expr::Call(c) = base else { break };
                    let inner = closure(&c.closures, None).map(|c| c.body.as_slice());
                    e = match inner {
                        Some([Stmt::Expr(inner)]) => Some(inner),
                        _ => break,
                    };
                }
                _ => break,
            }
        }
        if let Some(expr) = e {
            let (base, mods) = unwind(expr);
            if let Expr::Call(c) = base
                && ident(&c.callee) == Some("VStack")
            {
                let mut mods = Mods(mods);
                let explicit = self.take_explicit_id(&mut mods);
                let path = "/screen".to_owned();
                root.id = Some(self.take_id(explicit, "screen", "", &path));
                let path = element_path("", &root);
                let generated = matches!(implicit(arg(&c.args, "alignment")), Some("leading"))
                    && c.args.len() == 1;
                if !generated && !c.args.is_empty() {
                    self.lose(
                        LossKind::Layout,
                        &path,
                        "the root stack's alignment and spacing are not kept",
                    );
                }
                let body = closure(&c.closures, None)
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                self.depth = 1;
                root.children = self.views(&body, Place::Nodes, &path);
                self.common(&mut root, mods, &path);
                return root;
            }
        }
        root.id = Some(self.take_id(None, "screen", "", "/screen"));
        let path = element_path("", &root);
        self.lose(
            LossKind::Structure,
            &path,
            "the view's body is not one stack; a screen root is added around it",
        );
        self.depth = 1;
        root.children = self.views(statements, Place::Nodes, &path);
        root
    }

    pub(super) fn take_explicit_id(&mut self, mods: &mut Mods) -> Option<String> {
        let m = mods.take("accessibilityIdentifier")?;
        match first_arg(&m.args) {
            Some(Expr::Str(p)) => plain_string(p),
            _ => None,
        }
    }

    /// The views of a view builder, in order.
    pub(super) fn views(&mut self, statements: &[Stmt], place: Place, path: &str) -> Vec<Child> {
        let mut out = vec![];
        for s in statements {
            if self.truncated {
                break;
            }
            match s {
                Stmt::Expr(e) => out.extend(self.view(e, place, path)),
                Stmt::If {
                    condition,
                    then,
                    otherwise,
                } => out.extend(self.conditional(condition, then, otherwise, place, path)),
                Stmt::Other(kind) => {
                    self.lose(
                        LossKind::Structure,
                        path,
                        format!("a `{kind}` in a view builder is not read"),
                    );
                }
            }
        }
        out
    }

    /// `if c { a } else { b }`: both branches, each hidden while its condition is false.
    pub(super) fn conditional(
        &mut self,
        condition: &Expr,
        then: &[Stmt],
        otherwise: &[Stmt],
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let cond = match self.value(condition, Leaf::Bool, path) {
            Some(Value::Bind { bind, not }) => Some((bind, not)),
            _ => None,
        };
        if cond.is_none() {
            self.lose(LossKind::Hidden, path, "an `if` condition that is not a data path is not kept; its first branch is imported as shown");
        }
        let mut out = vec![];
        for (branch, shown_when) in [(then, false), (otherwise, true)] {
            if branch.is_empty() {
                continue;
            }
            if cond.is_none() && shown_when {
                break;
            }
            for mut child in self.views(branch, place, path) {
                if let (Some((bind, not)), Child::Node(n)) = (&cond, &mut child) {
                    if n.props.contains_key("hidden") {
                        self.lose(
                            LossKind::Hidden,
                            &element_path(path, n),
                            "an element hidden both by `if` and by its own condition keeps its own",
                        );
                    } else {
                        // Hidden when the branch's condition does not hold.
                        n.props.insert(
                            "hidden".to_owned(),
                            Value::Bind {
                                bind: bind.clone(),
                                not: !(*not ^ shown_when),
                            },
                        );
                    }
                }
                out.push(child);
            }
        }
        out
    }

    pub(super) fn enter(&mut self) -> bool {
        self.elements += 1;
        if self.elements > MAX_NODES || self.depth > MAX_DEPTH {
            self.truncated = true;
            return false;
        }
        true
    }

    pub(super) fn view(&mut self, expr: &Expr, place: Place, path: &str) -> Vec<Child> {
        let (base, mods) = unwind(expr);
        let mut mods = Mods(mods);
        let (name, call) = match base {
            Expr::Call(c) => (type_name(&c.callee).unwrap_or_default(), Some(c)),
            other => (type_name(other).unwrap_or_default(), None),
        };
        let empty = Call {
            callee: Box::new(Expr::Other(String::new())),
            args: vec![],
            closures: vec![],
        };
        let call = call.unwrap_or(&empty);
        // A sheet presented from the view is a dialog next to it.
        let sheets = mods.take_all("sheet");
        let alerts: Vec<Mod> = ["alert", "confirmationDialog"]
            .iter()
            .flat_map(|n| mods.take_all(n))
            .collect();
        let anchor = name == "Color.clear" && !sheets.is_empty();
        // A `ForEach` repeats children of the view around it; anything else starts its own.
        let row_child = self.row_child;
        if name != "ForEach" {
            self.row_child = false;
        }
        let grow = row_child && name != "ForEach" && !anchor && take_grow(&mut mods);
        let mut out = if anchor {
            vec![]
        } else {
            self.base_view(&name, call, &mut mods, place, path)
        };
        self.row_child = row_child;
        if grow && let Some(Child::Node(n)) = out.first_mut() {
            n.props.insert("grow".to_owned(), Value::Bool(true));
        }
        let anchor_hidden = if anchor {
            mods.take("weftHidden").or_else(|| mods.take("hidden"))
        } else {
            None
        };
        if anchor {
            mods.take("frame");
            self.ignored(mods, path);
        }
        for sheet in sheets {
            if let Some(dialog) = self.sheet(&sheet, anchor_hidden.as_ref(), path) {
                out.push(Child::Node(Box::new(dialog)));
            }
        }
        for alert in alerts {
            if let Some(dialog) = self.alert(&alert, path) {
                out.push(Child::Node(Box::new(dialog)));
            }
        }
        out
    }

    pub(super) fn base_view(
        &mut self,
        name: &str,
        call: &Call,
        mods: &mut Mods,
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let mods = Mods(std::mem::take(&mut mods.0));
        let children = |c: &Call| {
            closure(&c.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default()
        };
        match (name, place) {
            ("ForEach", _) => self.each(call, mods, place, path),
            (
                "Group" | "WeftCell" | "NavigationStack" | "NavigationView" | "ScrollView"
                | "LazyVStack" | "ZStack",
                _,
            ) => {
                // `WeftCell` only keeps a grid cell whole (`generate`).
                if !matches!(name, "Group" | "WeftCell") {
                    self.lose(
                        LossKind::Layout,
                        path,
                        format!("`{name}` is not kept; its content is"),
                    );
                }
                self.ignored(mods, path);
                self.views(&children(call), place, path)
            }
            ("Spacer" | "Divider" | "Color.clear", _) => {
                self.lose(LossKind::Layout, path, format!("`{name}` is not kept"));
                vec![]
            }
            ("EmptyView", _) => vec![],
            ("Text" | "Label", Place::Choice(kind)) => self.text_kind(kind, call, mods, path),
            ("Text" | "Label", Place::Header) => self.text_kind("column", call, mods, path),
            ("Button", Place::Menu) => self.button("menu-item", call, mods, path),
            (_, Place::List) if name != "HStack" => self.wrapped("item", name, call, mods, path),
            (_, Place::Row) if name != "HStack" => self.wrapped("cell", name, call, mods, path),
            ("HStack", Place::List) => self.mixed("item", call, mods, path),
            ("HStack", Place::Row) => self.mixed("cell", call, mods, path),
            ("GridRow", Place::Table) => self.grid_row(call, mods, path),
            (_, Place::Table) => self.wrapped("row", name, call, mods, path),
            (_, Place::Tabs) => self.tab(name, call, mods, path),
            (_, Place::Choice(_) | Place::Menu | Place::Header) => {
                self.lose(
                    LossKind::Structure,
                    path,
                    format!("`{name}` cannot go here in Weft and is dropped"),
                );
                vec![]
            }
            _ => self.element_view(name, call, mods, path),
        }
    }

    pub(super) fn element_view(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        match name {
            "HStack" if mods.has("weftCombobox") => self.combobox(call, mods, path),
            "VStack" | "HStack" | "LazyHStack" => self.stack(name, call, mods, path),
            "LazyVGrid" | "LazyHGrid" | "WeftColumns" => self.grid(name, call, mods, path),
            "Grid" => self.container("table", call, mods, Place::Table, path),
            "Section" => self.section(call, mods, path),
            "GroupBox"
                if call.args.is_empty() && closure(&call.closures, Some("label")).is_none() =>
            {
                self.mixed("alert", call, mods, path)
            }
            "GroupBox" => self.section(call, mods, path),
            "Text" | "Label" => {
                let heading = mods.has("accessibilityHeading")
                    || mods.0.iter().any(|m| {
                        m.name == "accessibilityAddTraits"
                            && implicit(first_arg(&m.args)) == Some("isHeader")
                    });
                let font = mods
                    .0
                    .iter()
                    .find(|m| m.name == "font")
                    .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
                let title_font = matches!(
                    font.as_deref(),
                    Some("largeTitle" | "title" | "title2" | "title3")
                );
                if title_font && !heading {
                    self.lose(
                        LossKind::Props,
                        path,
                        "a text with a title font is read as a heading",
                    );
                }
                let kind = if heading || title_font {
                    "heading"
                } else {
                    "text"
                };
                self.text_kind(kind, call, mods, path)
            }
            "Image" | "AsyncImage" => self.image(name, call, mods, path),
            "WeftModel" => self.model(call, mods, path),
            "Button" => {
                let link = mods.0.iter().any(|m| {
                    m.name == "accessibilityAddTraits"
                        && implicit(first_arg(&m.args)) == Some("isLink")
                });
                self.button(if link { "link" } else { "button" }, call, mods, path)
            }
            "Link" => self.button("link", call, mods, path),
            "Form" => self.form(call, mods, path),
            "TextField" | "SecureField" | "TextEditor" => self.field(name, call, mods, path),
            "Toggle" => self.toggle(call, mods, path),
            "Slider" => self.slider(call, mods, path),
            "Stepper" => self.stepper(call, mods, path),
            "DatePicker" => self.date_picker(call, mods, path),
            "ColorPicker" => self.color_picker(call, mods, path),
            "Picker" => self.picker(call, mods, path),
            "List" => self.list(call, mods, path),
            "TabView" => self.tabs(call, mods, path),
            "Menu" => self.menu(call, mods, path),
            _ => {
                if let Some(kind) = self.custom.get(name).cloned() {
                    return self.custom_view(&kind, call, mods, path);
                }
                if let Some(body) = self.views.get(name).copied()
                    && !self.inlining.iter().any(|v| v == name)
                {
                    // A helper view declared in the same file is read in place, its arguments
                    // standing for its properties.
                    let aliases = call
                        .args
                        .iter()
                        .filter_map(|a| Some((a.label.clone()?, a.value.clone())))
                        .collect();
                    self.aliases.push(aliases);
                    self.inlining.push(name.to_owned());
                    let out = self.views(body, Place::Nodes, path);
                    self.inlining.pop();
                    self.aliases.pop();
                    self.ignored_quiet(&mut mods);
                    self.ignored(mods, path);
                    return out;
                }
                let shown = if name.is_empty() {
                    "an expression"
                } else {
                    name
                };
                self.lose(
                    LossKind::Kinds,
                    path,
                    format!("`{shown}` has no Weft kind and is dropped"),
                );
                vec![]
            }
        }
    }

    /// A new element: id from the modifiers, then the shared modifiers once its content is read.
    pub(super) fn element(
        &mut self,
        kind: &str,
        mods: &mut Mods,
        name: &str,
        path: &str,
    ) -> Option<(Node, String)> {
        if !self.enter() {
            return None;
        }
        let explicit = self.take_explicit_id(mods);
        let mut node = Node::new(kind);
        let here = format!("{path}/{kind}");
        node.id = Some(self.take_id(explicit, kind, name, &here));
        let here = element_path(path, &node);
        Some((node, here))
    }

    pub(super) fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.depth += 1;
        let out = f(self);
        self.depth -= 1;
        out
    }

    /// Modifiers every kind shares: label, hidden, markers and handlers, then losses for the rest.
    pub(super) fn common(&mut self, node: &mut Node, mut mods: Mods, path: &str) {
        if let Some(m) = mods.take("accessibilityLabel")
            && let Some(e) = first_arg(&m.args)
        {
            match self.value(e, Leaf::Text, path) {
                Some(v) => {
                    node.props.insert("label".to_owned(), v);
                }
                None => self.lose(
                    LossKind::Names,
                    path,
                    "the accessibility label is not text or a data path",
                ),
            }
        }
        for name in ["weftHidden", "hidden"] {
            if let Some(m) = mods.take(name) {
                let v = match first_arg(&m.args) {
                    Some(e) => self.value(e, Leaf::Bool, path),
                    None => Some(Value::Bool(true)),
                };
                if let Some(v) = v {
                    node.props.insert("hidden".to_owned(), v);
                }
            }
        }
        for m in mods.take_all("rotation3DEffect") {
            self.tilt(node, &m.args, path);
        }
        if let Some(m) = mods.take("disabled")
            && let Some(v) = first_arg(&m.args).and_then(|e| self.value(e, Leaf::Bool, path))
        {
            if self.declares_prop(&node.kind, "disabled") {
                node.props.insert("disabled".to_owned(), v);
            } else {
                self.lose(
                    LossKind::Props,
                    path,
                    format!("`{}` cannot be disabled in Weft", node.kind),
                );
            }
        }
        for (modifier, event) in [
            ("onTapGesture", "press"),
            ("onSubmit", "submit"),
            ("onChange", "change"),
        ] {
            for m in mods.take_all(modifier) {
                let body = m
                    .closures
                    .first()
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                self.event(node, event, &body, path);
            }
        }
        for m in mods.take_all("weftProp") {
            let (Some(Expr::Str(name)), Some(e)) = (
                m.args.first().map(|a| &a.value),
                m.args.get(1).map(|a| &a.value),
            ) else {
                continue;
            };
            let Some(name) = plain_string(name) else {
                continue;
            };
            let Some((leaf, _)) = prop_leaf(self.catalog, &node.kind, &name) else {
                self.lose(
                    LossKind::Props,
                    path,
                    format!("`{}` has no `{name}` attribute", node.kind),
                );
                continue;
            };
            match self.value(e, leaf, path) {
                Some(v) => {
                    node.props.insert(name, v);
                }
                None => self.lose(
                    LossKind::Bindings,
                    path,
                    format!("the value of `{name}` is not a literal or a data path"),
                ),
            }
        }
        self.ignored_quiet(&mut mods);
        self.ignored(mods, path);
    }

    /// One `rotation3DEffect` of a tilt: the turn about an axis, and the perspective in pixels.
    pub(super) fn tilt(&mut self, node: &mut Node, args: &[Arg], path: &str) {
        let number = |e: &Expr| match e {
            Expr::Num(n) => Some(*n),
            Expr::Neg(inner) => match inner.as_ref() {
                Expr::Num(n) => Some(-n),
                _ => None,
            },
            _ => None,
        };
        let degrees = match first_arg(args) {
            Some(Expr::Call(c)) if matches!(c.callee.as_ref(), Expr::Implicit(n) if n == "degrees") => {
                first_arg(&c.args).and_then(number)
            }
            _ => None,
        };
        let axis = match arg(args, "axis") {
            Some(Expr::Tuple(items)) => ["x", "y", "z"].map(|name| {
                items
                    .iter()
                    .find(|a| a.label.as_deref() == Some(name))
                    .and_then(|a| number(&a.value))
            }),
            _ => [None; 3],
        };
        let perspective = match arg(args, "perspective") {
            Some(Expr::Num(n)) if *n == 0.0 => Some(None),
            Some(Expr::Binary { left, op, right })
                if op == "/"
                    && matches!(left.as_ref(), Expr::Num(n) if *n == crate::generate::TILT_NOMINAL_VIEW) =>
            {
                number(right).map(Some)
            }
            _ => None,
        };
        let (Some(degrees), [Some(x), Some(y), Some(z)], Some(perspective)) =
            (degrees, axis, perspective)
        else {
            self.lose(
                LossKind::Props,
                path,
                "a rotation3DEffect is not a tilt Weft can read",
            );
            return;
        };
        if let Some(p) = perspective {
            node.props
                .insert("perspective".to_owned(), Value::Number(p));
        }
        let name = match (x, y, z) {
            (0.0, 0.0, 0.0) => return,
            (1.0, 0.0, 0.0) => "rotate-x",
            (0.0, 1.0, 0.0) => "rotate-y",
            (0.0, 0.0, 1.0) => "rotate-z",
            _ => {
                self.lose(
                    LossKind::Props,
                    path,
                    "a rotation3DEffect turns about an axis Weft has no attribute for",
                );
                return;
            }
        };
        node.props.insert(name.to_owned(), Value::Number(degrees));
    }

    /// Modifiers `generate` adds for SwiftUI's sake, which carry nothing more.
    pub(super) fn ignored_quiet(&mut self, mods: &mut Mods) {
        mods.0.retain(|m| {
            let trait_arg = implicit(first_arg(&m.args));
            !matches!(
                (m.name.as_str(), trait_arg),
                ("accessibilityElement", _)
                    | (
                        "accessibilityAddTraits",
                        Some("isHeader" | "isModal" | "isLink")
                    )
                    | ("accessibilityRemoveTraits", Some("isButton"))
            )
        });
    }

    pub(super) fn ignored(&mut self, mods: Mods, path: &str) {
        let mut layout = vec![];
        let mut other = vec![];
        for m in mods.0 {
            if LAYOUT_MODIFIERS.contains(&m.name.as_str()) {
                if !layout.contains(&m.name) {
                    layout.push(m.name);
                }
            } else if !other.contains(&m.name) {
                other.push(m.name);
            }
        }
        if !layout.is_empty() {
            let list: Vec<String> = layout.iter().map(|n| format!("`.{n}`")).collect();
            self.lose(
                LossKind::Layout,
                path,
                format!("{} not kept", list.join(", ")),
            );
        }
        if !other.is_empty() {
            let list: Vec<String> = other.iter().map(|n| format!("`.{n}`")).collect();
            self.lose(
                LossKind::Props,
                path,
                format!("{} has no Weft form", list.join(", ")),
            );
        }
    }

    /// A stack's or grid's `.modifier(theme.<path>)`: the token is its material. Any other use of
    /// `modifier` stays for `ignored` to report.
    pub(super) fn material(&mut self, node: &mut Node, mods: &mut Mods) {
        for m in mods.take_all("modifier") {
            match first_arg(&m.args).and_then(|e| self.token(e)) {
                Some(t) => {
                    node.props.insert("material".to_owned(), Value::Token(t));
                }
                None => mods.0.push(m),
            }
        }
    }

    pub(super) fn finish(&mut self, mut node: Node, mods: Mods, path: &str) -> Vec<Child> {
        self.common(&mut node, mods, path);
        vec![Child::Node(Box::new(node))]
    }
}
