use std::rc::Rc;

use serde_json::{Map, Value as Json};
use weft_core::{Content, is_loop_variable};
use weft_import::js_trim;

use super::model::{Ctx, Leaf, MakeFn, MapFn, N, Piece, Scope, Var};
use super::names::BUSY_STATES;
use super::tree::{
    Attr, AttrV, C, Code, Seg, attr, attr_js, attr_s, el, expr, js, lit, s, str_js, str_literal,
};
use super::value::{E, FALSE, TRUE, dyn_e, lit_e, str_v};
use super::{Gen, MAX_DEPTH};
use crate::js::{V, compare_utf16, quote};

impl<'a> Gen<'a> {
    // ---- Pieces: `each` and `hidden` as the renderer's expansion reads them ----

    pub(super) fn pieces(
        &mut self,
        list: Option<&'a Json>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> Vec<Piece<'a>> {
        match list {
            Some(Json::Array(items)) => self.pieces_of(items, scope, depth),
            _ => Vec::new(),
        }
    }

    // A stack frame per nesting level stays small only when the compiler does not merge the
    // per-kind code into this function; see `render`.
    #[inline(never)]
    pub(super) fn pieces_of(
        &mut self,
        items: &'a [Json],
        scope: &Rc<Scope>,
        depth: usize,
    ) -> Vec<Piece<'a>> {
        let mut out: Vec<Piece<'a>> = Vec::new();
        if depth > MAX_DEPTH {
            return out;
        }
        for child in items {
            if let Json::String(text) = child {
                if js_trim(text).is_empty() {
                    continue;
                }
                // Adjacent text is one DOM text node; the renderer joins it with a space.
                if let Some(Piece::Text(last)) = out.last_mut() {
                    last.push(' ');
                    last.push_str(text);
                } else {
                    out.push(Piece::Text(text.clone()));
                }
                continue;
            }
            let Json::Object(record) = child else {
                continue;
            };
            let Some(Json::String(kind)) = record.get("kind") else {
                continue;
            };
            let props = record.get("props").and_then(Json::as_object);
            if kind == "each" {
                if let Some(each) = self.each(record, props, scope, depth) {
                    out.push(each);
                }
                continue;
            }
            let resolved = self.resolve(props.and_then(|p| p.get("hidden")), scope).0;
            let hidden = self.flag(resolved);
            if hidden.is(&TRUE) {
                continue;
            }
            let n = self.node(record, kind, props, scope, depth);
            out.push(Piece::Node(
                Rc::new(n),
                hidden.lit.is_none().then_some(hidden.js),
            ));
        }
        out
    }

    pub(super) fn each(
        &mut self,
        node: &'a Map<String, Json>,
        props: Option<&'a Map<String, Json>>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> Option<Piece<'a>> {
        let (source, source_path) = self.resolve(props.and_then(|p| p.get("in")), scope);
        let as_ = props.and_then(|p| p.get("as")).and_then(Json::as_str);
        let (Some(mut path), Some(as_)) = (source_path, as_) else {
            return None;
        };
        if !is_loop_variable(as_) {
            return None;
        }
        let index = format!("_i{}", scope.loops);
        // SolidJS's `<For>` passes the index as an accessor.
        let index_ref = if self.solid() {
            format!("{index}()")
        } else {
            index.clone()
        };
        path.push(s("."));
        path.push(js(index_ref.clone()));
        let v = Var {
            ident: self.ident(as_),
            path: path.clone(),
        };
        let ident = v.ident.clone();
        let mut vars = scope.vars.clone();
        vars.insert(as_.to_owned(), v);
        let mut suffix = scope.suffix.clone();
        suffix.extend([s("["), js(index_ref), s("]")]);
        let inner = Rc::new(Scope {
            vars,
            suffix,
            prefix: scope.prefix.clone(),
            item: Some(path),
            loops: scope.loops + 1,
        });
        let list = self.use_("_list");
        let src = format!("{list}({})", source.js);
        Some(Piece::Each {
            src,
            ident,
            index,
            inner: self.pieces(node.get("children"), &inner, depth + 1),
        })
    }

    pub(super) fn node(
        &self,
        raw: &'a Map<String, Json>,
        kind: &'a str,
        props: Option<&'a Map<String, Json>>,
        scope: &Rc<Scope>,
        depth: usize,
    ) -> N<'a> {
        let doc_id = raw.get("id").and_then(Json::as_str).unwrap_or("");
        let mut id = scope.prefix.clone();
        if !doc_id.is_empty() {
            id.push(s(doc_id));
        }
        id.extend(scope.suffix.iter().cloned());
        N {
            kind,
            def: self.catalog.components.get(kind),
            doc_id,
            id,
            props,
            on: raw.get("on").and_then(Json::as_object),
            children: raw.get("children"),
            slots: raw.get("slots").and_then(Json::as_object),
            scope: scope.clone(),
            depth,
        }
    }

    pub(super) fn declares_slot(&self, n: &N<'a>, name: &str) -> bool {
        n.def.and_then(|d| d.slot(name)).is_some()
    }

    /// A component's content as render.ts `content` places it (SPEC §4.2): the header slot first,
    /// then the default content, then every other slot by name, each named slot in its own box. A
    /// declared `empty` slot is left to the kind that shows it.
    #[inline(never)]
    pub(super) fn content(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> Vec<C<'a>> {
        let body_ctx = Ctx {
            grow: (n.kind == "stack").then(|| self.grow_axis(n)),
            ..ctx.clone()
        };
        // A slot's content is not a child of the stack (SPEC §2.2), so it does not grow.
        let slot_ctx = Ctx {
            grow: None,
            ..ctx.clone()
        };
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut names: Vec<&'a String> = n.slots.map(|s| s.keys().collect()).unwrap_or_default();
        names.sort_by(|a, b| compare_utf16(a, b));
        for name in names {
            if name == "empty" && self.declares_slot(n, name) {
                continue;
            }
            let pieces = self.pieces(n.slot(name), &n.scope, n.depth + 1);
            let kids = self.kids(&pieces, &slot_ctx, None);
            let slot = C::J(el(
                "div",
                vec![attr_s("data-weft-slot", name.as_str())],
                kids,
            ));
            if name == "header" {
                before.push(slot);
            } else {
                after.push(slot);
            }
        }
        let own = match n.def.map(|d| d.content) {
            Some(Content::Text | Content::Mixed) => self.text_prop(n),
            _ => Vec::new(),
        };
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let body = self.kids(&pieces, &body_ctx, None);
        before.extend(own);
        before.extend(body);
        before.extend(after);
        before
    }

    /// Whether the default slot has something to show besides empty loops, as render-react's
    /// `shows` decides it (SPEC §5.1 `empty`): a static element counts even when hidden, a loop
    /// only when its array has items, and table columns never count.
    pub(super) fn filled(&mut self, n: &N<'a>) -> E {
        let mut loops = Vec::new();
        if let Some(Json::Array(items)) = n.children {
            for c in items {
                let Some(record) = c.as_object() else {
                    continue;
                };
                let Some(kind) = record.get("kind").and_then(Json::as_str) else {
                    continue;
                };
                if kind == "column" {
                    continue;
                }
                if kind != "each" {
                    return lit_e(TRUE);
                }
                let source_raw = record
                    .get("props")
                    .and_then(Json::as_object)
                    .and_then(|p| p.get("in"));
                let source = self.resolve(source_raw, &n.scope).0;
                match &source.lit {
                    None => {
                        let list = self.use_("_list");
                        loops.push(format!("{list}({}).length > 0", source.js));
                    }
                    Some(V::Arr(len)) if *len > 0 => return lit_e(TRUE),
                    Some(_) => {}
                }
            }
        }
        if loops.is_empty() {
            lit_e(FALSE)
        } else {
            dyn_e(loops.join(" || "), true)
        }
    }

    /// Whether a declared `empty` slot replaces the default content (render-react
    /// `showsEmpty`).
    pub(super) fn shows_empty(&mut self, n: &N<'a>) -> E {
        if !self.declares_slot(n, "empty") || n.slot("empty").is_none() {
            return lit_e(FALSE);
        }
        let state = self.text_of(n, "state");
        let filled = self.filled(n);
        if state.is(&str_v("empty")) || filled.is(&FALSE) {
            return lit_e(TRUE);
        }
        if state.lit.is_some() && filled.lit.is_some() {
            return lit_e(FALSE);
        }
        let mut parts = Vec::new();
        if state.lit.is_none() {
            parts.push(format!("{} === \"empty\"", state.js));
        }
        if filled.lit.is_none() {
            parts.push(format!("!({})", filled.js));
        }
        dyn_e(parts.join(" || "), true)
    }

    /// `then` when `when` holds, else `otherwise` (nothing when absent).
    pub(super) fn cond(&mut self, when: &str, then: C<'a>, otherwise: Option<C<'a>>) -> C<'a> {
        if self.solid() {
            self.solid.insert("Show");
            let mut attrs = vec![attr_js("when", when)];
            if let Some(o) = otherwise {
                attrs.push(attr("fallback", AttrV::Expr(Box::new(o))));
            }
            return C::J(el("Show", attrs, vec![then]));
        }
        let mut segs = vec![lit(format!("{when} ? ")), expr(then), lit(" : ")];
        match otherwise {
            Some(o) => segs.push(expr(o)),
            None => segs.push(lit("null")),
        }
        C::Code(Code::of(segs))
    }

    /// Pieces as JSX children.
    #[inline(never)]
    pub(super) fn kids(
        &mut self,
        list: &[Piece<'a>],
        ctx: &Ctx<'a>,
        mut map: Option<&mut MapFn<'_, 'a>>,
    ) -> Vec<C<'a>> {
        let mut out = Vec::new();
        for p in list {
            match p {
                Piece::Text(text) => {
                    let c = match map.as_deref_mut() {
                        Some(f) => f(self, Leaf::Text(text)),
                        None => Some(C::Text(text.clone())),
                    };
                    out.extend(c);
                }
                Piece::Node(n, hidden) => {
                    if map.is_none() && !n.doc_id.is_empty() {
                        out.extend(self.comments(n.doc_id));
                    }
                    let c = match map.as_deref_mut() {
                        Some(f) => f(self, Leaf::Node(n)),
                        None => self.render(n, ctx),
                    };
                    let Some(c) = c else {
                        continue;
                    };
                    match hidden {
                        None => out.push(c),
                        Some(h) if self.keyless() => {
                            out.push(self.cond(&format!("!{h}"), c, None));
                        }
                        Some(h) => {
                            out.push(C::Code(Code::of(vec![lit(format!("!{h} && ")), expr(c)])))
                        }
                    }
                }
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.kids(inner, ctx, map.as_deref_mut());
                    if self.solid() {
                        self.solid.insert("For");
                        let frag = el("", vec![], inner);
                        let body = Code::of(vec![
                            lit(format!("({ident}, {index}) => ")),
                            expr(C::J(frag)),
                        ]);
                        out.push(C::J(el(
                            "For",
                            vec![attr_js("each", src.as_str())],
                            vec![C::Code(body)],
                        )));
                    } else if self.lit() {
                        let frag = el("", vec![], inner);
                        out.push(C::Code(Code::of(vec![
                            lit(format!("{src}.map(({ident}, {index}) => ")),
                            expr(C::J(frag)),
                            lit(")"),
                        ])));
                    } else {
                        self.fragment = true;
                        let frag = el("Fragment", vec![attr_js("key", index.as_str())], inner);
                        out.push(C::Code(Code::of(vec![
                            lit(format!("{src}.map(({ident}, {index}) => ")),
                            expr(C::J(frag)),
                            lit(")"),
                        ])));
                    }
                }
            }
        }
        out
    }

    /// Pieces as an array expression, keeping the entries `make` returns.
    #[inline(never)]
    pub(super) fn collect(&mut self, list: &[Piece<'a>], make: &mut MakeFn<'_, 'a>) -> Code<'a> {
        let mut items = Vec::new();
        for p in list {
            let (leaf, hidden) = match p {
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.collect(inner, make);
                    let head = self.loop_call("flatMap", src, ident, index, "");
                    items.push(Code::of(vec![
                        lit(format!("...{head}")),
                        Seg::Code(inner),
                        lit(")"),
                    ]));
                    continue;
                }
                Piece::Text(text) => (Leaf::Text(text), None),
                Piece::Node(n, hidden) => (Leaf::Node(n), hidden.as_ref()),
            };
            let Some(item) = make(self, leaf) else {
                continue;
            };
            items.push(match hidden {
                None => item,
                Some(h) => Code::of(vec![
                    lit(format!("...({h} ? [] : [")),
                    Seg::Code(item),
                    lit("])"),
                ]),
            });
        }
        Code::of(vec![Seg::Array(items)])
    }

    // ---- Attributes ----

    pub(super) fn attr(&self, attrs: &mut Vec<Attr<'a>>, name: &str, e: E) {
        match e.lit {
            Some(V::Undef | V::Null) => {}
            Some(V::Str(text)) => attrs.push(attr(name, AttrV::S(text))),
            Some(v) => attrs.push(attr(name, AttrV::Js(v.literal()))),
            None => attrs.push(attr(name, AttrV::Js(e.js))),
        }
    }

    /// An HTML boolean attribute: present when true, absent otherwise.
    pub(super) fn bool_attr(&self, attrs: &mut Vec<Attr<'a>>, name: &str, e: E) {
        match e.lit {
            None => attrs.push(attr(name, AttrV::Js(e.js))),
            Some(V::Bool(true)) => attrs.push(attr(name, AttrV::True)),
            Some(_) => {}
        }
    }

    pub(super) fn or_undefined(&self, e: E) -> E {
        match e.lit {
            Some(V::Str(text)) if text.is_empty() => lit_e(V::Undef),
            Some(v) => lit_e(v),
            None => dyn_e(format!("{} || undefined", e.js), false),
        }
    }

    /// The document id and state on every element that carries a declared role (render.ts
    /// `base`).
    #[inline(never)]
    pub(super) fn base(&mut self, n: &N<'a>, named: bool) -> Vec<Attr<'a>> {
        let mut a = Vec::new();
        if !n.doc_id.is_empty() {
            a.push(attr(
                "data-weft-id",
                match str_literal(&n.id) {
                    Some(text) => AttrV::S(text),
                    None => AttrV::Js(str_js(&n.id)),
                },
            ));
        }
        let state = self.text_of(n, "state");
        let shown = self.or_undefined(state.clone());
        self.attr(&mut a, "data-state", shown);
        match &state.lit {
            Some(v) => {
                if BUSY_STATES.contains(&v.string().as_str()) {
                    a.push(attr_s("aria-busy", "true"));
                }
            }
            None => {
                let busy = self.use_("_busy");
                a.push(attr_js("aria-busy", format!("{busy}({})", state.js)));
            }
        }
        if named {
            let label = self.label(n);
            let shown = self.or_undefined(label);
            self.attr(&mut a, "aria-label", shown);
        }
        a
    }

    pub(super) fn fire(&mut self, n: &N<'a>, event: &str) -> Option<String> {
        let action = n.event(event)?;
        // `{$back}` in a fragment body is the action parameter, not an action name.
        let action_js = action
            .strip_prefix("{$")
            .and_then(|rest| rest.strip_suffix('}'))
            .and_then(|name| n.scope.vars.get(name).map(|v| v.ident.clone()))
            .unwrap_or_else(|| quote(action));
        let item = n
            .scope
            .item
            .as_ref()
            .map(|i| format!(", item: {}", str_js(i)))
            .unwrap_or_default();
        let act = self.use_("_act");
        Some(format!(
            "{act}({}, {{ id: {}, action: {}{item} }})",
            self.actions_ident(),
            str_js(&n.id),
            action_js
        ))
    }

    pub(super) fn write(&mut self, n: &N<'a>, name: &str, value: &str) -> Option<String> {
        let path = self.prop(n, name).1?;
        Some(format!(
            "{}?.({}, {value})",
            self.on_change_ident(),
            str_js(&path)
        ))
    }

    pub(super) fn handler(params: &str, statements: Vec<Option<String>>) -> String {
        let body: Vec<String> = statements.into_iter().flatten().collect();
        match body.as_slice() {
            [] => format!("{params} => {{}}"),
            [only] => format!("{params} => {only}"),
            _ => format!("{params} => {{ {}; }}", body.join("; ")),
        }
    }

    pub(super) fn pressable(&mut self, n: &N<'a>, attrs: &mut Vec<Attr<'a>>) {
        let Some(fire) = self.fire(n, "press") else {
            return;
        };
        let press = self.use_("_press");
        attrs.push(attr_js(self.an("tabIndex"), "0"));
        attrs.push(attr_js("onClick", format!("() => {fire}")));
        attrs.push(attr_js(
            "onKeyDown",
            format!("(_e) => {press}(_e, () => {fire})"),
        ));
    }
}
