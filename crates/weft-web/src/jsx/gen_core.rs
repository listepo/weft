use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value as Json;
use weft_core::{Catalog, PropType, is_binding};
use weft_import::{js_trim, squash};

use super::model::{N, Piece, Scope, content_text};
use super::names::RESERVED;
use super::tree::{C, Code, Str, s};
use super::value::{E, FALSE, TRUE, clamp_number, dyn_e, lit_e, opt_literal, str_v};
use super::{Framework, Gen};
use crate::js::{INHERITED, V, is_plain_segment, quote};

impl<'a> Gen<'a> {
    pub(super) fn new(catalog: &'a Catalog, fw: Framework, ts: bool) -> Self {
        Gen {
            catalog,
            fw,
            ts,
            used: HashSet::new(),
            fragment: false,
            inline_names: HashSet::new(),
            in_inline: false,
            solid: BTreeSet::new(),
            solid_web: BTreeSet::new(),
            lit_uses: super::lit::Uses::default(),
            notes: HashMap::new(),
        }
    }

    /// `{/* … */}` children for the notes about `id`.
    pub(super) fn comments(&self, id: &str) -> Vec<C<'a>> {
        self.notes
            .get(id)
            .into_iter()
            .flatten()
            .map(|line| C::Code(Code::lit(format!("/* {line} */"))))
            .collect()
    }

    pub(super) fn solid(&self) -> bool {
        self.fw == Framework::Solid
    }

    pub(super) fn lit(&self) -> bool {
        self.fw == Framework::Lit
    }

    /// The event a text control reports a change on: React's `onChange` fires on every input,
    /// while SolidJS and Lit keep the DOM's meaning, where that event is `input`.
    pub(super) fn input_event(&self) -> &'static str {
        if self.solid() || self.lit() {
            "onInput"
        } else {
            "onChange"
        }
    }

    /// Loops and select options go without keys and `_keyed` fragments.
    pub(super) fn keyless(&self) -> bool {
        self.solid() || self.lit()
    }

    pub(super) fn use_(&mut self, helper: &'static str) -> &'static str {
        self.used.insert(helper);
        // A helper calls the ones it is built from, so using it brings them along.
        let built_from: &[&'static str] = match helper {
            "_span" | "_bounds" => &["_numeric"],
            "_slide" => &["_numeric", "_tidy"],
            "_count" => &["_numeric", "_clamp"],
            "_step" => &["_tidy", "_clamp"],
            _ => &[],
        };
        self.used.extend(built_from.iter().copied());
        if helper == "_keyed" {
            self.fragment = true;
        }
        helper
    }

    pub(super) fn data_ident(&self) -> &'static str {
        if self.solid() { "props.data" } else { "data" }
    }

    pub(super) fn actions_ident(&self) -> &'static str {
        if self.solid() {
            "props.actions"
        } else {
            "actions"
        }
    }

    pub(super) fn on_change_ident(&self) -> &'static str {
        if self.solid() {
            "props.onChange"
        } else {
            "onChange"
        }
    }

    pub(super) fn ident(&self, name: &str) -> String {
        if RESERVED.contains(&name) || (self.solid() && name == "props") {
            format!("{name}_")
        } else {
            name.to_owned()
        }
    }

    /// An attribute name as the framework spells it: SolidJS and Lit set HTML attributes by their
    /// HTML names and have no uncontrolled `default*` props.
    pub(super) fn an(&self, react: &'static str) -> &'static str {
        if !self.keyless() {
            return react;
        }
        match react {
            "tabIndex" => "tabindex",
            "colSpan" => "colspan",
            "defaultValue" => "value",
            "defaultChecked" => "checked",
            other => other,
        }
    }

    /// A style key: React's camelCase property names, SolidJS's CSS property names.
    pub(super) fn css(&self, react: &str, css: &str) -> String {
        if self.solid() && react != css {
            quote(css)
        } else {
            react.to_owned()
        }
    }

    // ---- Values (SPEC §2.1, as resolved by the renderer) ----

    pub(super) fn resolve(&mut self, raw: Option<&Json>, scope: &Scope) -> (E, Option<Str>) {
        let Some(Json::Object(record)) = raw else {
            return (lit_e(V::of(raw)), None);
        };
        let Some(Json::String(bind)) = record.get("bind") else {
            return (lit_e(V::Undef), None);
        };
        if !is_binding(bind) {
            return (lit_e(V::Undef), None);
        }
        let (name, rest) = match bind.find('.') {
            Some(dot) => (&bind[1..dot], &bind[dot..]),
            None => (&bind[1..], ""),
        };
        let segments: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest[1..].split('.').collect()
        };
        let (base, path) = if name.is_empty() {
            (self.data_ident().to_owned(), vec![s("$")])
        } else {
            let Some(v) = scope.vars.get(name) else {
                return (lit_e(V::Undef), None);
            };
            (v.ident.clone(), v.path.clone())
        };
        let access = if segments
            .iter()
            .all(|s| is_plain_segment(s) && INHERITED.binary_search(s).is_err())
        {
            let mut a = base;
            for s in &segments {
                a.push_str("?.");
                a.push_str(s);
            }
            a
        } else {
            let get = self.use_("_get");
            format!("{get}({base}, {})", weft_core::to_compact(&segments))
        };
        if record.get("not") == Some(&Json::Bool(true)) {
            let on = self.use_("_on");
            return (dyn_e(format!("!{on}({access})"), true), None);
        }
        let mut full = path;
        full.push(s(rest));
        (dyn_e(access, false), Some(full))
    }

    pub(super) fn text(&mut self, e: E) -> E {
        match &e.lit {
            Some(v) => lit_e(str_v(v.text())),
            None => {
                let f = self.use_("_text");
                dyn_e(format!("{f}({})", e.js), false)
            }
        }
    }

    pub(super) fn flag(&mut self, e: E) -> E {
        if let Some(v) = &e.lit {
            return lit_e(V::Bool(v.truthy()));
        }
        if e.bool {
            return e;
        }
        let f = self.use_("_on");
        dyn_e(format!("{f}({})", e.js), true)
    }

    /// A number prop is brought into its declared range as the renderer's `prop` does (SPEC §5.1).
    pub(super) fn prop(&mut self, n: &N<'a>, name: &str) -> (E, Option<Str>) {
        let (e, path) = self.resolve(n.raw(name), &n.scope);
        let Some(def) = n
            .def
            .and_then(|d| d.prop(name))
            .filter(|d| d.kind == PropType::Number)
        else {
            return (e, path);
        };
        let integer = def.integer == Some(true);
        if let Some(v) = &e.lit {
            return (lit_e(clamp_number(v, integer, def.min, def.max)), path);
        }
        let bounds = [
            V::Bool(integer).literal(),
            opt_literal(def.min),
            opt_literal(def.max),
        ]
        .join(", ");
        let f = self.use_("_num");
        (dyn_e(format!("{f}({}, {bounds})", e.js), false), path)
    }

    pub(super) fn text_of(&mut self, n: &N<'a>, name: &str) -> E {
        let e = self.prop(n, name).0;
        self.text(e)
    }

    pub(super) fn flag_of(&mut self, n: &N<'a>, name: &str) -> E {
        let e = self.prop(n, name).0;
        self.flag(e)
    }

    pub(super) fn label(&mut self, n: &N<'a>) -> E {
        let t = self.text_of(n, "label");
        match &t.lit {
            Some(v) => lit_e(str_v(js_trim(&v.string()))),
            None => dyn_e(format!("{}.trim()", t.js), false),
        }
    }

    /// SPEC §5.1: a text-bearing kind takes its text from content or from the `text` prop. As in
    /// render-react's expansion, the prop counts only when the content renders nothing (content
    /// wins when a lenient reader is given both) and its text is not blank.
    #[inline(never)]
    pub(super) fn text_prop_shown(&mut self, n: &N<'a>) -> (E, E) {
        let text = self.text_of(n, "text");
        if n.raw("text").is_none() {
            return (text, lit_e(FALSE));
        }
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let content = self.any_shown(&pieces);
        let blank = match &text.lit {
            Some(v) => lit_e(V::Bool(js_trim(&v.text()).is_empty())),
            None => dyn_e(format!("{}.trim() === \"\"", text.js), false),
        };
        if content.is(&TRUE) || blank.is(&TRUE) {
            return (text, lit_e(FALSE));
        }
        let parts: Vec<&str> = [&content, &blank]
            .into_iter()
            .filter(|e| e.lit.is_none())
            .map(|e| e.js.as_str())
            .collect();
        let when = if parts.is_empty() {
            lit_e(TRUE)
        } else {
            dyn_e(format!("!({})", parts.join(" || ")), true)
        };
        (text, when)
    }

    /// The visible text of a `text`-content kind, joined and squashed as `contentText` does.
    pub(super) fn own_text(&mut self, n: &N<'a>) -> E {
        let content = lit_e(str_v(content_text(n.children)));
        let (text, when) = self.text_prop_shown(n);
        if let Some(w) = &when.lit {
            return if *w == TRUE {
                self.squash(text)
            } else {
                content
            };
        }
        let squashed = self.squash(text);
        dyn_e(
            format!("{} ? {} : {}", when.js, squashed.js, content.js),
            false,
        )
    }

    pub(super) fn squash(&mut self, e: E) -> E {
        match &e.lit {
            Some(v) => lit_e(str_v(squash(&v.text()))),
            None => {
                let f = self.use_("_squash");
                dyn_e(format!("{f}({})", e.js), false)
            }
        }
    }

    /// A loop over `src` outside JSX, with the loop variable and index in scope of `body`.
    pub(super) fn loop_call(
        &mut self,
        method: &str,
        src: &str,
        ident: &str,
        index: &str,
        body: &str,
    ) -> String {
        if self.solid() {
            let ix = self.use_("_ix");
            format!("{ix}({src}).{method}(([{ident}, {index}]) => {body}")
        } else {
            format!("{src}.{method}(({ident}, {index}) => {body}")
        }
    }

    /// Whether any of these pieces renders, i.e. whether the renderer's expanded list is
    /// non-empty.
    #[inline(never)]
    pub(super) fn any_shown(&mut self, list: &[Piece<'a>]) -> E {
        let mut parts = Vec::new();
        for p in list {
            match p {
                Piece::Text(_) | Piece::Node(_, None) => return lit_e(TRUE),
                Piece::Node(_, Some(hidden)) => parts.push(format!("!{hidden}")),
                Piece::Each {
                    src,
                    ident,
                    index,
                    inner,
                } => {
                    let inner = self.any_shown(inner);
                    if inner.is(&FALSE) {
                        continue;
                    }
                    let call = self.loop_call("some", src, ident, index, &inner.js);
                    parts.push(format!("{call})"));
                }
            }
        }
        if parts.is_empty() {
            lit_e(FALSE)
        } else {
            dyn_e(parts.join(" || "), true)
        }
    }
}
