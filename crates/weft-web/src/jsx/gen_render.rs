use std::rc::Rc;

use serde_json::Value as Json;
use weft_core::{ARIA_ROLES, is_token, js_number};

use super::Gen;
use super::helpers::{block, css_var, grows, justify_css, outermost};
use super::model::{Ctx, Grow, N};
use super::names::ALIGN;
use super::tree::{Attr, AttrV, C, Code, J, attr_js, attr_s, el};
use super::value::{E, TRUE, str_v};
use crate::html::model_asset_url;
use crate::js::{V, is_integer, quote};
use crate::tilt;

impl<'a> Gen<'a> {
    // ---- Kinds (render.ts `renderNode`) ----

    // Every kind gets its own frame rather than one frame as large as all of them together, so
    // that a document nested to the depth limit fits the stack of WebAssembly and of threads.
    #[inline(never)]
    pub(super) fn render(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        let out = self.render_kind(n, ctx)?;
        Some(self.grown(n, ctx, Self::tilted(n, out)))
    }

    /// SPEC §2.2: the tilt is a `transform` on the element the node draws (a dialog, drawn into
    /// the top layer, is the one exception, as in the reference renderer). The props are literals,
    /// so the value is written out.
    pub(super) fn tilted(n: &N<'a>, mut out: C<'a>) -> C<'a> {
        if n.kind == "dialog" {
            return out;
        }
        let transform = tilt::css(|name| n.raw(name).and_then(Json::as_f64));
        if let Some(transform) = transform {
            Self::push_style(&mut out, &[format!("transform: {}", quote(&transform))]);
        }
        out
    }

    /// `grow` on the element the node draws, the same flex declarations the static page's
    /// `data-grow` rule applies. A dialog is drawn into the top layer, as for the tilt.
    pub(super) fn grown(&self, n: &N<'a>, ctx: &Ctx<'a>, mut out: C<'a>) -> C<'a> {
        let Some(axis) = &ctx.grow else {
            return out;
        };
        if n.kind == "dialog" || !grows(n) {
            return out;
        }
        let flex = self.css("flex", "flex");
        let min_w = self.css("minWidth", "min-width");
        let min_h = self.css("minHeight", "min-height");
        // A string zero, so the declaration is `min-width: 0` (a number would become `0px`).
        let entries = match axis {
            Grow::Row => vec![format!("{flex}: \"1 1 0%\""), format!("{min_w}: \"0\"")],
            Grow::Column => vec![format!("{flex}: \"1 1 0%\""), format!("{min_h}: \"0\"")],
            Grow::Bound(js) => vec![
                format!("{flex}: \"1 1 0%\""),
                format!("{min_w}: {js} === \"row\" ? \"0\" : undefined"),
                format!("{min_h}: {js} === \"row\" ? undefined : \"0\""),
            ],
        };
        Self::push_style(&mut out, &entries);
        out
    }

    pub(super) fn grow_axis(&mut self, n: &N<'a>) -> Grow {
        let direction = self.text_of(n, "direction");
        match &direction.lit {
            Some(v) if *v == str_v("row") => Grow::Row,
            Some(_) => Grow::Column,
            None => Grow::Bound(direction.js.clone()),
        }
    }

    /// Appends declarations to the outermost element's style. A block's result is that element
    /// (a link computes its href before the tag), so the declarations land on the tag.
    pub(super) fn push_style(out: &mut C<'_>, entries: &[String]) {
        if entries.is_empty() {
            return;
        }
        let entry = entries.join(", ");
        let Some(j) = outermost(out) else {
            return;
        };
        match j.attrs.iter_mut().find(|a| a.name == "style") {
            Some(Attr {
                value: AttrV::Js(code),
                ..
            }) => match code.strip_suffix(" }") {
                Some(open) => *code = format!("{open}, {entry} }}"),
                None => *code = format!("{{ ...{code}, {entry} }}"),
            },
            _ => j.attrs.push(attr_js("style", format!("{{ {entry} }}"))),
        }
    }

    #[inline(never)]
    pub(super) fn render_kind(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        if let Some(call) = self.inline_use(n, ctx) {
            return Some(call);
        }
        if n.kind == "outlet" {
            let name = n.raw("name").and_then(Json::as_str).unwrap_or("");
            let base = if self.solid() { "props.slots" } else { "slots" };
            return Some(C::Code(Code::lit(format!("{base}?.{name}"))));
        }
        if n.def.is_none() {
            return Some(self.fallback(n, ctx));
        }
        Some(match n.kind {
            "screen" => {
                let a = self.base(n, true);
                C::J(el("main", a, self.content(n, ctx)))
            }
            "stack" | "grid" => {
                let mut a = self.base(n, false);
                let style = self.layout_style(n);
                a.push(attr_js("style", style));
                if n.raw("material").is_some_and(|m| m.get("token").is_some()) {
                    a.push(attr_s("data-weft-material", ""));
                }
                C::J(el("div", a, self.content(n, ctx)))
            }
            "section" => {
                let a = self.base(n, true);
                C::J(el("section", a, self.content(n, ctx)))
            }
            "heading" => self.heading(n),
            "text" => {
                let mut a = self.base(n, false);
                let tone = self.text_of(n, "tone");
                let tone = self.or_undefined(tone);
                self.attr(&mut a, "data-tone", tone);
                let text = self.own_text(n);
                C::J(el("div", a, self.text_kids(text)))
            }
            "image" => {
                let mut a = self.base(n, false);
                let label = self.label(n);
                self.attr(&mut a, "alt", label);
                let src = self.text_of(n, "src");
                let url = self.use_("_url");
                a.push(attr_js("src", format!("{url}({})", src.js)));
                C::J(el("img", a, vec![]))
            }
            "model" => self.model(n),
            "link" => self.link(n),
            "button" => self.button(n, ctx),
            "form" => {
                let mut a = self.base(n, true);
                let submit = self.fire(n, "submit");
                a.push(attr_js(
                    "onSubmit",
                    Self::handler("(_e)", vec![Some("_e.preventDefault()".to_owned()), submit]),
                ));
                let inner = Ctx {
                    form: Some(n.clone()),
                    ..ctx.clone()
                };
                C::J(el("form", a, self.content(n, &inner)))
            }
            "field" => self.field(n),
            "checkbox" | "switch" => self.toggle(n),
            "radio-group" | "segmented-control" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "radiogroup"));
                if n.kind == "segmented-control" {
                    a.push(attr_s("data-weft-segmented", ""));
                }
                let label = self.label(n);
                let mut kids = self.caption(label);
                let inner = Ctx {
                    group: Some(n.clone()),
                    ..ctx.clone()
                };
                kids.extend(self.content(n, &inner));
                C::J(el("div", a, kids))
            }
            "radio" | "segment" => self.radio(n, ctx),
            "select" => self.select(n),
            "slider" => self.slider(n),
            "stepper" => self.stepper(n),
            "date-picker" => self.date_picker(n),
            "color-picker" => self.color_picker(n),
            "combobox" => self.combobox(n),
            "option" => {
                let a = self.base(n, false);
                let text = self.own_text(n);
                C::J(el("div", a, self.text_kids(text)))
            }
            "list" => self.list(n, ctx),
            "item" => {
                let mut a = self.base(n, true);
                self.pressable(n, &mut a);
                C::J(el("li", a, self.content(n, ctx)))
            }
            "table" => self.table(n, ctx),
            "tabs" => self.tabs(n, ctx),
            "tab" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "group"));
                C::J(el("div", a, self.content(n, ctx)))
            }
            "dialog" => return self.dialog(n, ctx),
            "alert" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "alert"));
                let tone = self.text_of(n, "tone");
                let tone = self.or_undefined(tone);
                self.attr(&mut a, "data-tone", tone);
                C::J(el("div", a, self.content(n, ctx)))
            }
            "menu" => {
                let mut a = self.base(n, true);
                a.push(attr_s("role", "menu"));
                C::J(el("div", a, self.content(n, ctx)))
            }
            "menu-item" => {
                let mut a = self.base(n, true);
                a.push(attr_s("type", "button"));
                a.push(attr_s("role", "menuitem"));
                let disabled = self.flag_of(n, "disabled");
                self.bool_attr(&mut a, "disabled", disabled);
                if let Some(fire) = self.fire(n, "press") {
                    a.push(attr_js("onClick", format!("() => {fire}")));
                }
                let text = self.own_text(n);
                C::J(el("button", a, self.text_kids(text)))
            }
            _ => self.fallback(n, ctx),
        })
    }

    /// `<model-viewer>` with its still as the poster child (SPEC §9). The paths are literals the
    /// generator checks itself, so the component carries no URL guard for them.
    pub(super) fn model(&mut self, n: &N<'a>) -> C<'a> {
        let mut a = self.base(n, true);
        a.push(attr_s("role", "img"));
        let label = self.label(n);
        self.attr(&mut a, "alt", label);
        let path = |prop: &str| {
            n.raw(prop)
                .and_then(Json::as_str)
                .and_then(|value| model_asset_url(prop, value))
        };
        if let Some(src) = path("src") {
            a.push(attr_s("src", src));
        }
        if let Some(usdz) = path("usdz") {
            a.push(attr_s("ios-src", usdz));
        }
        a.push(attr_s("camera-controls", ""));
        a.push(attr_s("interaction-prompt", "none"));
        let poster = path("fallback").map(|still| {
            C::J(el(
                "img",
                vec![
                    attr_s("slot", "poster"),
                    attr_s("alt", ""),
                    attr_s("src", still),
                ],
                vec![],
            ))
        });
        C::J(el("model-viewer", a, poster.into_iter().collect()))
    }

    pub(super) fn text_kids(&self, e: E) -> Vec<C<'a>> {
        match &e.lit {
            Some(V::Str(text)) if text.is_empty() => vec![],
            Some(v) => vec![C::Text(v.string())],
            None => vec![C::Code(Code::lit(e.js))],
        }
    }

    /// The visible caption of a control (render.ts `caption`): the name itself is `aria-label`,
    /// so the caption is hidden from the tree rather than read twice, and absent when there is
    /// none.
    pub(super) fn caption(&mut self, name: E) -> Vec<C<'a>> {
        let span = el(
            "span",
            vec![attr_s("aria-hidden", "true")],
            self.text_kids(name.clone()),
        );
        match &name.lit {
            Some(V::Str(text)) if text.is_empty() => vec![],
            Some(_) => vec![C::J(span)],
            None => vec![self.cond(&name.js, C::J(span), None)],
        }
    }

    /// The `text` prop as raw content, where a kind's content is rendered as child nodes
    /// (render.ts `kids`) rather than joined like `contentText`.
    pub(super) fn text_prop(&mut self, n: &N<'a>) -> Vec<C<'a>> {
        let (text, when) = self.text_prop_shown(n);
        match &when.lit {
            Some(w) if *w == TRUE => self.text_kids(text),
            Some(_) => vec![],
            None if self.solid() => {
                vec![self.cond(&when.js, C::Code(Code::lit(text.js)), None)]
            }
            None => vec![C::Code(Code::lit(format!(
                "{} ? {} : null",
                when.js, text.js
            )))],
        }
    }

    /// Unknown kinds and extensions keep their children under their fallback role (SPEC §8).
    #[inline(never)]
    pub(super) fn fallback(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> C<'a> {
        let declared = match n.def {
            Some(def) => Some(def.role.as_str()),
            None => n.raw("role").and_then(Json::as_str),
        };
        let role = declared
            .filter(|r| ARIA_ROLES.contains(r))
            .unwrap_or("group");
        let kids = self.content(n, ctx);
        if matches!(role, "none" | "presentation" | "generic") {
            let a = self.base(n, false);
            return C::J(el("div", a, kids));
        }
        let mut a = self.base(n, true);
        if role == "region" || role == "form" {
            // Unnamed, both are generic (HTML-AAM), so the role is kept only with a label.
            let label = self.label(n);
            match &label.lit {
                Some(V::Str(text)) if text.is_empty() => {}
                Some(_) => a.push(attr_s("role", role)),
                None => a.push(attr_js(
                    "role",
                    format!("{} ? {} : undefined", label.js, quote(role)),
                )),
            }
        } else {
            a.push(attr_s("role", role));
        }
        C::J(el("div", a, kids))
    }

    #[inline(never)]
    pub(super) fn layout_style(&mut self, n: &N<'a>) -> String {
        let mut entries = Vec::new();
        if let Some(token) = n
            .raw("gap")
            .and_then(Json::as_object)
            .and_then(|g| g.get("token"))
            .and_then(Json::as_str)
            .filter(|t| is_token(t))
        {
            let value = format!("var(--weft-{})", token.replace('.', "-"));
            entries.push(format!("gap: {}", quote(&value)));
        }
        if let Some(token) = n
            .raw("material")
            .and_then(Json::as_object)
            .and_then(|g| g.get("token"))
            .and_then(Json::as_str)
            .filter(|t| is_token(t))
        {
            for prop in crate::html::MATERIAL_PROPS {
                let value = format!("var(--weft-{}-{prop})", token.replace('.', "-"));
                entries.push(format!("\"--_weft-material-{prop}\": {}", quote(&value)));
            }
        }
        self.box_style(n, &mut entries);
        if n.kind == "grid" {
            entries.push("display: \"grid\"".to_owned());
            let columns = self.prop(n, "columns").0;
            let key = self.css("gridTemplateColumns", "grid-template-columns");
            let min = css_var(n, "min-column-width");
            let gap = css_var(n, "gap");
            match &columns.lit {
                Some(V::Num(c)) if is_integer(*c) && (1.0..=64.0).contains(c) => {
                    let value = match &min {
                        Some(min) => crate::html::capped_columns(*c as u32, min, gap.as_deref()),
                        None => format!("repeat({}, minmax(0, 1fr))", js_number(*c)),
                    };
                    entries.push(format!("{key}: {}", quote(&value)));
                }
                Some(_) => {}
                None => {
                    if let Some(min) = min {
                        let fill = self.use_("_fill");
                        let gap = gap
                            .map(|g| quote(&g))
                            .unwrap_or_else(|| "undefined".to_owned());
                        entries.push(format!(
                            "{key}: {fill}({}, {}, {gap})",
                            columns.js,
                            quote(&min)
                        ));
                    } else {
                        let cols = self.use_("_cols");
                        entries.push(format!("{key}: {cols}({})", columns.js));
                    }
                }
            }
            return format!("{{ {} }}", entries.join(", "));
        }
        entries.push("display: \"flex\"".to_owned());
        let direction = self.text_of(n, "direction");
        let key = self.css("flexDirection", "flex-direction");
        entries.push(match &direction.lit {
            Some(v) => format!(
                "{key}: {}",
                if *v == str_v("row") {
                    "\"row\""
                } else {
                    "\"column\""
                }
            ),
            None => format!("{key}: {} === \"row\" ? \"row\" : \"column\"", direction.js),
        });
        let align = self.text_of(n, "align");
        let key = self.css("alignItems", "align-items");
        // SPEC §5.1: a row without a valid `align` centres its children across the row.
        let row_default = match &direction.lit {
            Some(v) => (*v == str_v("row")).then(|| "center".to_owned()),
            None => Some(format!(
                "{} === \"row\" ? \"center\" : undefined",
                direction.js
            )),
        };
        match &align.lit {
            Some(v) => {
                let k = v.string();
                if let Some((_, value)) = ALIGN.iter().find(|(name, _)| *name == k) {
                    entries.push(format!("{key}: {}", quote(value)));
                } else if let Some(d) = row_default {
                    let d = if d == "center" { quote(&d) } else { d };
                    entries.push(format!("{key}: {d}"));
                }
            }
            None => {
                let f = self.use_("_align");
                entries.push(format!("{key}: {f}({}, {})", align.js, direction.js));
            }
        }
        let wrap = self.flag_of(n, "wrap");
        let key = self.css("flexWrap", "flex-wrap");
        match &wrap.lit {
            None => entries.push(format!("{key}: {} ? \"wrap\" : undefined", wrap.js)),
            Some(v) if *v == TRUE => entries.push(format!("{key}: \"wrap\"")),
            Some(_) => {}
        }
        self.justify_style(n, &mut entries);
        format!("{{ {} }}", entries.join(", "))
    }

    /// `padding` and `max-width`, the same inline declarations the static page writes.
    pub(super) fn box_style(&self, n: &N<'a>, entries: &mut Vec<String>) {
        if let Some(value) = css_var(n, "padding") {
            let key = self.css("padding", "padding");
            entries.push(format!("{key}: {}", quote(&value)));
        }
        if let Some(value) = css_var(n, "max-width") {
            let width = self.css("width", "width");
            let max = self.css("maxWidth", "max-width");
            let box_ = self.css("boxSizing", "box-sizing");
            entries.push(format!("{width}: \"100%\""));
            entries.push(format!("{max}: {}", quote(&value)));
            // The cap is the border box, so padding stays inside it.
            entries.push(format!("{box_}: \"border-box\""));
        }
    }

    /// `start` is the initial value, so it is not written (SPEC §5.1).
    pub(super) fn justify_style(&mut self, n: &N<'a>, entries: &mut Vec<String>) {
        let justify = self.text_of(n, "justify");
        let key = self.css("justifyContent", "justify-content");
        match &justify.lit {
            Some(v) => {
                if let Some(css) = justify_css(&v.string()) {
                    entries.push(format!("{key}: {}", quote(css)));
                }
            }
            None => {
                let f = self.use_("_justify");
                entries.push(format!("{key}: {f}({})", justify.js));
            }
        }
    }

    /// An element whose tag is chosen at run time; `cast` is the tag TSX checks its attributes
    /// against.
    pub(super) fn with_tag(&mut self, tag_js: String, cast: &str, mut j: J<'a>) -> C<'a> {
        if self.lit() {
            // Lit's static `html` tag takes the element name as a static value; the expression
            // only ever yields a name from the closed set the catalog allows.
            self.lit_uses.static_html = true;
            j.tag = "${Tag}".to_owned();
            return block(
                vec![Code::lit(format!("const Tag = unsafeStatic({tag_js});"))],
                C::J(j),
            );
        }
        if self.solid() {
            self.solid_web.insert("Dynamic");
            j.tag = "Dynamic".to_owned();
            j.attrs.insert(0, attr_js("component", tag_js));
            return C::J(j);
        }
        let line = if self.ts {
            format!("const Tag = ({tag_js}) as \"{cast}\";")
        } else {
            format!("const Tag = {tag_js};")
        };
        j.tag = "Tag".to_owned();
        block(vec![Code::lit(line)], C::J(j))
    }
}
