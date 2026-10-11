use std::rc::Rc;

use serde_json::Value as Json;
use weft_core::js_number;
use weft_import::js_number_from;

use super::Gen;
use super::helpers::block;
use super::model::{Ctx, Leaf, N, is_binding_raw};
use super::names::INPUT_TYPES;
use super::tree::{Attr, AttrV, C, Code, J, Seg, attr, attr_js, attr_s, el, expr, lit, s, str_js};
use super::value::{E, FALSE, dyn_e, lit_e, str_v};
use crate::js::{V, is_integer};

impl<'a> Gen<'a> {
    #[inline(never)]
    pub(super) fn heading(&mut self, n: &N<'a>) -> C<'a> {
        let a = self.base(n, true);
        let text = self.own_text(n);
        let j = el("", a, self.text_kids(text));
        let level = self.prop(n, "level").0;
        let Some(v) = &level.lit else {
            let f = self.use_("_level");
            return self.with_tag(format!("\"h\" + {f}({})", level.js), "h2", j);
        };
        let num = match v {
            V::Str(text) => Some(js_number_from(text)),
            V::Num(x) => Some(*x),
            _ => None,
        };
        let h = num
            .filter(|x| is_integer(*x) && (1.0..=6.0).contains(x))
            .unwrap_or(2.0);
        C::J(J {
            tag: format!("h{}", js_number(h)),
            ..j
        })
    }

    #[inline(never)]
    pub(super) fn link(&mut self, n: &N<'a>) -> C<'a> {
        let fire = self.fire(n, "press");
        let mut a = self.base(n, true);
        a.push(attr_js("href", "_href"));
        // Without an href an <a> is neither a link nor focusable; both are restored, and like a
        // native link it then activates on Enter only.
        a.push(attr_js(
            "role",
            "_href === undefined ? \"link\" : undefined",
        ));
        a.push(attr_js(
            self.an("tabIndex"),
            "_href === undefined ? 0 : undefined",
        ));
        if let Some(fire) = fire {
            a.push(attr_js(
                "onKeyDown",
                format!(
                    "_href === undefined ? (_e) => {{ if (_e.key !== \"Enter\" || _e.target !== _e.currentTarget) return; _e.preventDefault(); {fire}; }} : undefined"
                ),
            ));
            a.push(attr_js("onClick", format!("() => {fire}")));
        }
        a.push(attr_js("style", "{ display: \"inline-block\" }"));
        let href = self.text_of(n, "href");
        let url = self.use_("_url");
        let line = format!("const _href = {url}({});", href.js);
        let text = self.own_text(n);
        let kids = self.text_kids(text);
        block(vec![Code::lit(line)], C::J(el("a", a, kids)))
    }

    #[inline(never)]
    pub(super) fn button(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> C<'a> {
        // `submit` is literal-only (SPEC §5.1), so a binding never turns a button into a submit
        // button.
        let submit = matches!(n.raw("submit"), Some(Json::Bool(true)))
            || matches!(n.raw("submit"), Some(Json::String(s)) if s == "true");
        let mut a = self.base(n, true);
        a.push(attr_s("type", if submit { "submit" } else { "button" }));
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let variant = self.text_of(n, "variant");
        let variant = self.or_undefined(variant);
        self.attr(&mut a, "data-variant", variant);
        // As in render.ts, the browser's own submission (also from Enter in a field) is cancelled
        // and reported here, so press and the form's submit each fire exactly once, in that order.
        let form = if submit { ctx.form.clone() } else { None };
        let press = self.fire(n, "press");
        let send = match &form {
            Some(f) => self.fire(f, "submit"),
            None => None,
        };
        let click = vec![
            submit.then(|| "_e.preventDefault()".to_owned()),
            press,
            send,
        ];
        if click.iter().any(Option::is_some) {
            a.push(attr_js("onClick", Self::handler("(_e)", click)));
        }
        let text = self.own_text(n);
        C::J(el("button", a, self.text_kids(text)))
    }

    #[inline(never)]
    pub(super) fn field(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let error = self.text_of(n, "error");
        let ty = self.text_of(n, "type");
        let mut error_parts = vec![s("weft-")];
        error_parts.extend(n.id.iter().cloned());
        error_parts.push(s("-error"));
        let error_id = str_js(&error_parts);
        let value = self.text_of(n, "value");
        let mut a = self.base(n, false);
        self.attr(&mut a, "aria-label", name.clone());
        let mut tag = None;
        match &ty.lit {
            Some(v) => {
                let t = v.string();
                if t == "multiline" {
                    tag = Some("textarea");
                } else {
                    tag = Some("input");
                    let shown = if INPUT_TYPES.contains(&t.as_str()) {
                        t
                    } else {
                        "text".to_owned()
                    };
                    a.push(attr_s("type", shown));
                }
            }
            None => a.push(attr_js(
                "type",
                format!(
                    "{t} === \"multiline\" ? undefined : [\"text\", \"email\", \"password\", \"number\", \"search\"].includes({t}) ? {t} : \"text\"",
                    t = ty.js
                ),
            )),
        }
        let placeholder = self.text_of(n, "placeholder");
        let placeholder = self.or_undefined(placeholder);
        self.attr(&mut a, "placeholder", placeholder);
        let required = self.flag_of(n, "required");
        self.bool_attr(&mut a, "required", required);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let state = self.text_of(n, "state");
        match (&state.lit, &error.lit) {
            (Some(st), Some(err)) => {
                let err = err.string();
                if *st == str_v("invalid") || !err.is_empty() {
                    a.push(attr_s("aria-invalid", "true"));
                }
                if !err.is_empty() {
                    a.push(attr_js("aria-describedby", error_id.clone()));
                }
            }
            _ => {
                a.push(attr_js(
                    "aria-invalid",
                    format!(
                        "{} === \"invalid\" || {} !== \"\" ? \"true\" : undefined",
                        state.js, error.js
                    ),
                ));
                a.push(attr_js(
                    "aria-describedby",
                    format!("{} ? {error_id} : undefined", error.js),
                ));
            }
        }
        let change = self.fire(n, "change");
        // React's `onChange` on a text control fires on every input; SolidJS keeps the DOM's
        // meaning, where that event is `input`.
        let on_input = self.input_event();
        if is_binding_raw(n.raw("value")) {
            // A number input shows "" for anything that is not a valid float, so the value is
            // sanitized the same way before the framework sees it.
            let shown = match &ty.lit {
                Some(t) if *t == str_v("number") => {
                    let f = self.use_("_float");
                    format!("{f}({})", value.js)
                }
                Some(_) => value.js.clone(),
                None => {
                    let f = self.use_("_float");
                    format!(
                        "{} === \"number\" ? {f}({}) : {}",
                        ty.js, value.js, value.js
                    )
                }
            };
            a.push(attr_js("value", shown));
            let write = self.write(n, "value", "_e.currentTarget.value");
            a.push(attr_js(
                on_input,
                Self::handler("(_e)", vec![write, change]),
            ));
        } else {
            self.attr(&mut a, self.an("defaultValue"), value);
            if let Some(change) = change {
                a.push(attr_js(on_input, format!("() => {change}")));
            }
        }
        let control = el(tag.unwrap_or("Tag"), a, vec![]);
        let control = match tag {
            Some(_) => C::J(control),
            None => self.with_tag(
                format!("{} === \"multiline\" ? \"textarea\" : \"input\"", ty.js),
                "input",
                control,
            ),
        };
        let error_el = el(
            "div",
            vec![attr_js("id", error_id)],
            self.text_kids(error.clone()),
        );
        let mut label_kids = self.caption(name);
        label_kids.push(control);
        let mut kids = vec![C::J(el("label", vec![], label_kids))];
        match &error.lit {
            Some(v) => {
                if !v.string().is_empty() {
                    kids.push(C::J(error_el));
                }
            }
            None => kids.push(self.cond(&error.js, C::J(error_el), None)),
        }
        C::J(el("div", vec![attr_s("data-weft-field", "")], kids))
    }

    #[inline(never)]
    pub(super) fn toggle(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let mut a = self.base(n, false);
        a.push(attr_s("type", "checkbox"));
        if n.kind == "switch" {
            a.push(attr_s("role", "switch"));
        }
        self.attr(&mut a, "aria-label", name.clone());
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let checked = self.flag_of(n, "checked");
        let change = self.fire(n, "change");
        if is_binding_raw(n.raw("checked")) {
            a.push(attr_js("checked", checked.js));
            let write = self.write(n, "checked", "_e.currentTarget.checked");
            a.push(attr_js(
                "onChange",
                Self::handler("(_e)", vec![write, change]),
            ));
        } else {
            self.bool_attr(&mut a, self.an("defaultChecked"), checked);
            if let Some(change) = change {
                a.push(attr_js("onChange", format!("() => {change}")));
            }
        }
        let mut kids = vec![C::J(el("input", a, vec![]))];
        kids.extend(self.caption(name));
        C::J(el("label", vec![], kids))
    }

    #[inline(never)]
    pub(super) fn radio(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let group = ctx.group.clone();
        let label = self.label(n);
        let own = self.own_text(n);
        let (name, shown) = match (&label.lit, &own.lit) {
            (Some(l), Some(o)) => {
                let (l, o) = (l.string(), o.string());
                let name = if l.is_empty() { o.clone() } else { l.clone() };
                let shown = if o.is_empty() { l } else { o };
                (lit_e(str_v(name)), lit_e(str_v(shown)))
            }
            _ => (
                dyn_e(format!("{} || {}", label.js, own.js), false),
                dyn_e(format!("{} || {}", own.js, label.js), false),
            ),
        };
        let value = self.text_of(n, "value");
        let mut a = self.base(n, false);
        a.push(attr_s("type", "radio"));
        if let Some(g) = group.as_ref().filter(|g| !g.doc_id.is_empty()) {
            let mut parts = vec![s("weft-")];
            parts.extend(g.id.iter().cloned());
            a.push(attr_js("name", str_js(&parts)));
        }
        self.attr(&mut a, "value", value.clone());
        self.attr(&mut a, "aria-label", name);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let mut on = lit_e(FALSE);
        if let Some(g) = &group {
            let gv = self.text_of(g, "value");
            on = match (&gv.lit, &value.lit) {
                (Some(gv), Some(v)) => {
                    let gv = gv.string();
                    lit_e(V::Bool(!gv.is_empty() && gv == v.string()))
                }
                _ => dyn_e(
                    format!("{} !== \"\" && {} === {}", gv.js, value.js, gv.js),
                    true,
                ),
            };
        }
        let controlled = group
            .as_ref()
            .is_some_and(|g| is_binding_raw(g.raw("value")));
        if controlled {
            a.push(attr_js("checked", on.js));
        } else {
            self.bool_attr(&mut a, self.an("defaultChecked"), on);
        }
        if let Some(g) = &group {
            let own_value = self.prop(n, "value").0;
            let statements = vec![
                self.write(g, "value", &own_value.js),
                self.fire(g, "change"),
            ];
            if controlled || statements.iter().any(Option::is_some) {
                a.push(attr_js("onChange", Self::handler("()", statements)));
            }
        }
        // The visible text is the content; a `label` only overrides the accessible name.
        let span = el(
            "span",
            vec![attr_s("aria-hidden", "true")],
            self.text_kids(shown),
        );
        C::J(el(
            "label",
            vec![],
            vec![C::J(el("input", a, vec![])), C::J(span)],
        ))
    }

    /// The options of a `select` or `combobox` as the `{ value, el }` list the component reads:
    /// anything else inside is dropped, as the HTML parser would. `chosen` marks the selected one.
    #[inline(never)]
    pub(super) fn option_list(&mut self, n: &N<'a>, chosen: Option<E>) -> Code<'a> {
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        self.collect(&pieces, &mut |g, leaf| {
            let Leaf::Node(o) = leaf else {
                return None;
            };
            if o.kind != "option" {
                return None;
            }
            let mut a = g.base(o, true);
            let value = g.text_of(o, "value");
            if let Some(chosen) = &chosen {
                match (&value.lit, &chosen.lit) {
                    (Some(mine), Some(theirs)) => {
                        if mine.string() == theirs.string() {
                            a.push(attr("selected", AttrV::True));
                        }
                    }
                    _ => a.push(attr_js(
                        "selected",
                        format!("{} === {}", value.js, chosen.js),
                    )),
                }
            }
            g.attr(&mut a, "value", value);
            let text = g.own_text(o);
            let option = el("option", a, g.text_kids(text));
            Some(Code::of(vec![
                lit("{ value: "),
                Seg::Value(o.clone()),
                lit(", el: "),
                expr(C::J(option)),
                lit(" }"),
            ]))
        })
    }

    /// Only options are rendered: the HTML parser drops anything else inside <select>.
    #[inline(never)]
    pub(super) fn select(&mut self, n: &N<'a>) -> C<'a> {
        let value = self.text_of(n, "value");
        // SolidJS's server renderer writes a select's value as an attribute HTML ignores, so each
        // option says whether it is the chosen one; the select's own value stays for the client.
        let chosen = self.keyless().then(|| value.clone());
        let options = self.option_list(n, chosen);
        let mut a = self.base(n, false);
        let label = self.label(n);
        self.attr(&mut a, "aria-label", label);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let controlled = is_binding_raw(n.raw("value"));
        if controlled {
            a.push(attr_js("value", value.js));
        } else {
            self.attr(&mut a, self.an("defaultValue"), value);
        }
        let write = self.write(n, "value", "_c ? _c.value : _e.currentTarget.value");
        let change = self.fire(n, "change");
        if write.is_some() {
            let text = self.use_("_text");
            let find =
                format!("const _c = _o.find((_x) => {text}(_x.value) === _e.currentTarget.value)");
            a.push(attr_js(
                "onChange",
                Self::handler("(_e)", vec![Some(find), write, change]),
            ));
        } else if controlled || change.is_some() {
            a.push(attr_js("onChange", Self::handler("()", vec![change])));
        }
        let shown = if self.keyless() {
            "_o.map((_x) => _x.el)".to_owned()
        } else {
            let keyed = self.use_("_keyed");
            format!("{keyed}(_o.map((_x) => _x.el))")
        };
        let control = block(
            vec![Code::of(vec![
                lit("const _o = "),
                Seg::Code(options),
                lit(";"),
            ])],
            C::J(el("select", a, vec![C::Code(Code::lit(shown))])),
        );
        let label = self.label(n);
        let mut kids = self.caption(label);
        kids.push(control);
        C::J(el("label", vec![], kids))
    }

    /// The `value` of a number, date, colour or combobox control (render.ts `valued`): a bound
    /// one is controlled and writes what the reader gives, a literal one only starts there.
    /// `read` is the JavaScript that turns the DOM value into the data value; `undefined` means
    /// the input holds nothing that fits, so nothing is written.
    pub(super) fn valued(&mut self, n: &N<'a>, a: &mut Vec<Attr<'a>>, shown: String, read: &str) {
        let change = self.fire(n, "change");
        // React's `onChange` on these controls fires on every input; SolidJS keeps the DOM's
        // meaning, where that event is `input`.
        let on_input = self.input_event();
        if is_binding_raw(n.raw("value")) {
            a.push(attr_js("value", shown));
            let write = self
                .write(n, "value", "_v")
                .map(|w| format!("if (_v !== undefined) {w}"));
            // With nothing to write or fire the handler stays (React warns about a controlled
            // input without one), but a lone `const` is not a valid arrow body.
            let read = (write.is_some() || change.is_some()).then(|| format!("const _v = {read}"));
            a.push(attr_js(
                on_input,
                Self::handler("(_e)", vec![read, write, change]),
            ));
        } else {
            a.push(attr_js(self.an("defaultValue"), shown));
            if let Some(change) = change {
                a.push(attr_js(on_input, format!("() => {change}")));
            }
        }
    }

    /// The control inside its label, with the visible caption before it.
    pub(super) fn labelled(&mut self, n: &N<'a>, control: C<'a>) -> C<'a> {
        let name = self.label(n);
        let mut kids = self.caption(name);
        kids.push(control);
        C::J(el("label", vec![], kids))
    }

    #[inline(never)]
    pub(super) fn slider(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let bounds: Vec<String> = ["min", "max", "step"]
            .iter()
            .map(|p| self.prop(n, p).0.js)
            .collect();
        let value = self.prop(n, "value").0.js;
        let span = self.use_("_span");
        let slide = self.use_("_slide");
        let mut a = self.base(n, false);
        a.push(attr_s("type", "range"));
        self.attr(&mut a, "aria-label", name);
        for p in ["min", "max", "step"] {
            a.push(attr_js(p, format!("_s.{p}")));
        }
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        self.valued(
            n,
            &mut a,
            format!("{slide}({value}, _s)"),
            "_numeric(_e.currentTarget.value)",
        );
        let control = block(
            vec![Code::lit(format!(
                "const _s = {span}({});",
                bounds.join(", ")
            ))],
            C::J(el("input", a, vec![])),
        );
        self.labelled(n, control)
    }

    #[inline(never)]
    pub(super) fn stepper(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let bounds: Vec<String> = ["min", "max", "step"]
            .iter()
            .map(|p| self.prop(n, p).0.js)
            .collect();
        let value = self.prop(n, "value").0.js;
        let make = self.use_("_bounds");
        let count = self.use_("_count");
        let step = self.use_("_step");
        let numeric = self.use_("_numeric");
        let disabled = self.flag_of(n, "disabled");
        let bound = is_binding_raw(n.raw("value"));
        // The buttons are for the pointer; the number input steps with the arrow keys already,
        // so they stay out of the tab order and the accessibility tree (APG spinbutton).
        let button = |g: &mut Self, direction: i32, which: &str, sign: &str| {
            let mut a = vec![
                attr_s("type", "button"),
                attr_js(g.an("tabIndex"), "-1"),
                attr_s("aria-hidden", "true"),
                attr_s("data-weft-step", which),
            ];
            g.bool_attr(&mut a, "disabled", disabled.clone());
            let set = (!bound).then(|| "if (_i) _i.value = String(_n)".to_owned());
            a.push(attr_js(
                "onClick",
                Self::handler(
                    "(_e)",
                    vec![
                        Some("const _i = _e.currentTarget.parentElement?.querySelector(\"input\")".to_owned()),
                        Some(format!(
                            "const _n = {step}({numeric}(_i?.value) ?? {count}({value}, _b), {direction}, _b)"
                        )),
                        set,
                        g.write(n, "value", "_n"),
                        g.fire(n, "change"),
                    ],
                ),
            ));
            C::J(el("button", a, vec![C::Text(sign.to_owned())]))
        };
        let down = button(self, -1, "down", "\u{2212}");
        let up = button(self, 1, "up", "+");
        let mut a = self.base(n, false);
        a.push(attr_s("type", "number"));
        self.attr(&mut a, "aria-label", name);
        for p in ["min", "max", "step"] {
            a.push(attr_js(p, format!("_b.{p}")));
        }
        self.bool_attr(&mut a, "disabled", disabled);
        self.valued(
            n,
            &mut a,
            format!("String({count}({value}, _b))"),
            &format!("{numeric}(_e.currentTarget.value)"),
        );
        let input = C::J(el("input", a, vec![]));
        let group = C::J(el(
            "span",
            vec![attr_s("data-weft-stepper", "")],
            vec![down, input, up],
        ));
        let control = block(
            vec![Code::lit(format!(
                "const _b = {make}({});",
                bounds.join(", ")
            ))],
            group,
        );
        self.labelled(n, control)
    }

    #[inline(never)]
    pub(super) fn date_picker(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let ty = self.text_of(n, "type").js;
        let kind = self.use_("_dateType");
        let date = self.use_("_dateText");
        let mut a = self.base(n, false);
        a.push(attr_js(
            "type",
            "_t === \"datetime\" ? \"datetime-local\" : _t",
        ));
        self.attr(&mut a, "aria-label", name);
        for p in ["min", "max"] {
            let text = self.text_of(n, p).js;
            a.push(attr_js(p, format!("{date}(_t, {text}) || undefined")));
        }
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        let value = self.text_of(n, "value").js;
        self.valued(
            n,
            &mut a,
            format!("{date}(_t, {value})"),
            &format!("{date}(_t, _e.currentTarget.value)"),
        );
        let control = block(
            vec![Code::lit(format!("const _t = {kind}({ty});"))],
            C::J(el("input", a, vec![])),
        );
        self.labelled(n, control)
    }

    #[inline(never)]
    pub(super) fn color_picker(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let color = self.use_("_color");
        let value = self.text_of(n, "value").js;
        let mut a = self.base(n, false);
        a.push(attr_s("type", "color"));
        self.attr(&mut a, "aria-label", name);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        self.valued(
            n,
            &mut a,
            format!("{color}({value})"),
            "_e.currentTarget.value",
        );
        let control = C::J(el("input", a, vec![]));
        self.labelled(n, control)
    }

    /// An editable combobox: the text input with a `datalist` of its options, linked by id.
    #[inline(never)]
    pub(super) fn combobox(&mut self, n: &N<'a>) -> C<'a> {
        let name = self.label(n);
        let value = self.text_of(n, "value").js;
        let options = self.option_list(n, None);
        let list = (!n.doc_id.is_empty()).then(|| {
            let mut parts = vec![s("weft-")];
            parts.extend(n.id.iter().cloned());
            parts.push(s("-options"));
            str_js(&parts)
        });
        let mut a = self.base(n, false);
        self.attr(&mut a, "aria-label", name.clone());
        if let Some(list) = &list {
            a.push(attr_js("list", list.clone()));
        }
        let placeholder = self.text_of(n, "placeholder");
        let placeholder = self.or_undefined(placeholder);
        self.attr(&mut a, "placeholder", placeholder);
        let disabled = self.flag_of(n, "disabled");
        self.bool_attr(&mut a, "disabled", disabled);
        self.valued(n, &mut a, value, "_e.currentTarget.value");
        let mut kids = self.caption(name);
        kids.push(C::J(el("input", a, vec![])));
        if let Some(list) = list {
            let shown = if self.keyless() {
                "_o.map((_x) => _x.el)".to_owned()
            } else {
                let keyed = self.use_("_keyed");
                format!("{keyed}(_o.map((_x) => _x.el))")
            };
            kids.push(C::J(el(
                "datalist",
                vec![attr_js("id", list)],
                vec![C::Code(Code::lit(shown))],
            )));
        }
        block(
            vec![Code::of(vec![
                lit("const _o = "),
                Seg::Code(options),
                lit(";"),
            ])],
            C::J(el("label", vec![], kids)),
        )
    }
}
