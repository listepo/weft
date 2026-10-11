use super::Gen;
use super::helpers::{block, cell_wrap};
use super::model::{Ctx, Leaf, N, filter_pieces};
use super::names::SORTS;
use super::tree::{Attr, AttrV, C, Code, J, Seg, attr, attr_js, attr_s, el, expr, lit, str_js};
use super::value::{FALSE, TRUE, lit_e};
use crate::js::quote;

impl<'a> Gen<'a> {
    /// SPEC §5.1: the `empty` slot is shown instead of the items when there are none, or when
    /// the state says `empty`.
    #[inline(never)]
    pub(super) fn list(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let ordered = self.flag_of(n, "ordered");
        let content = self.content(n, ctx);
        let show = self.shows_empty(n);
        let mut kids = content;
        if !show.is(&FALSE) {
            // A list may only hold list items, so the empty content stands in a presentational
            // one.
            let pieces = self.pieces(n.slot("empty"), &n.scope, n.depth + 1);
            let empty_kids = self.kids(&pieces, ctx, None);
            let empty = el(
                "li",
                vec![attr_s("role", "none"), attr_s("data-weft-slot", "empty")],
                empty_kids,
            );
            kids = if show.lit.is_some() {
                vec![C::J(empty)]
            } else {
                let items = el("", vec![], kids);
                vec![self.cond(&show.js, C::J(empty), Some(C::J(items)))]
            };
        }
        let a = self.base(n, true);
        let j = el("", a, kids);
        match &ordered.lit {
            Some(v) => C::J(J {
                tag: if *v == TRUE { "ol" } else { "ul" }.to_owned(),
                ..j
            }),
            None => self.with_tag(format!("{} ? \"ol\" : \"ul\"", ordered.js), "ul", j),
        }
    }

    /// SPEC §5.1: `column` children form the header row in <thead>; other content goes to
    /// <tbody>, wrapped in a row and cell when it is not a row.
    #[inline(never)]
    pub(super) fn table(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let children = self.pieces(n.children, &n.scope, n.depth + 1);
        let columns = self.collect(&children, &mut |g, leaf| match leaf {
            Leaf::Node(c) if c.kind == "column" => {
                let column = g.column(c);
                Some(Code::of(vec![expr(C::J(column))]))
            }
            _ => None,
        });
        let rows = self.collect(&children, &mut |g, leaf| {
            let c = match leaf {
                Leaf::Node(p) if p.kind == "column" => return None,
                Leaf::Text(text) => Some(C::J(el(
                    "tr",
                    vec![],
                    vec![C::J(el("td", vec![], vec![C::Text(text.to_owned())]))],
                ))),
                Leaf::Node(p) if p.kind == "row" => Some(C::J(g.row(p, ctx))),
                Leaf::Node(p) => {
                    let rendered = g.render(p, ctx);
                    cell_wrap(rendered)
                }
            };
            c.map(|c| Code::of(vec![expr(c)]))
        });
        let (cols, rows_js) = if self.keyless() {
            ("_cols".to_owned(), "_rows".to_owned())
        } else {
            let keyed = self.use_("_keyed");
            (format!("{keyed}(_cols)"), format!("{keyed}(_rows)"))
        };
        let head = el(
            "thead",
            vec![],
            vec![C::J(el("tr", vec![], vec![C::Code(Code::lit(cols))]))],
        );
        let body = el("tbody", vec![], vec![C::Code(Code::lit(rows_js))]);
        let mut kids = vec![self.cond("_cols.length > 0", C::J(head), None)];
        let show = self.shows_empty(n);
        if show.is(&FALSE) {
            kids.push(self.cond("_rows.length > 0", C::J(body), None));
        } else {
            // A table keeps its column headers; the empty content spans them in one body row.
            let pieces = self.pieces(n.slot("empty"), &n.scope, n.depth + 1);
            let empty_kids = self.kids(&pieces, ctx, None);
            let empty = el(
                "tbody",
                vec![],
                vec![C::J(el(
                    "tr",
                    vec![attr_s("data-weft-slot", "empty")],
                    vec![C::J(el(
                        "td",
                        vec![attr_js(self.an("colSpan"), "Math.max(1, _cols.length)")],
                        empty_kids,
                    ))],
                ))],
            );
            if show.lit.is_some() {
                kids.push(C::J(empty));
            } else {
                let rest = self.cond("_rows.length > 0", C::J(body), None);
                kids.push(self.cond(&show.js, C::J(empty), Some(rest)));
            }
        }
        let a = self.base(n, true);
        block(
            vec![
                Code::of(vec![lit("const _cols = "), Seg::Code(columns), lit(";")]),
                Code::of(vec![lit("const _rows = "), Seg::Code(rows), lit(";")]),
            ],
            C::J(el("table", a, kids)),
        )
    }

    #[inline(never)]
    pub(super) fn column(&mut self, n: &N<'a>) -> J<'a> {
        let mut a = self.base(n, true);
        a.push(attr_s("scope", "col"));
        let sort = self.text_of(n, "sort");
        match &sort.lit {
            Some(v) => {
                let v = v.string();
                if SORTS.contains(&v.as_str()) {
                    a.push(attr_s("aria-sort", v));
                }
            }
            None => a.push(attr_js(
                "aria-sort",
                format!(
                    "[\"none\", \"ascending\", \"descending\"].includes({s}) ? {s} : undefined",
                    s = sort.js
                ),
            )),
        }
        self.pressable(n, &mut a);
        let text = self.own_text(n);
        el("th", a, self.text_kids(text))
    }

    #[inline(never)]
    pub(super) fn row(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> J<'a> {
        let mut a = self.base(n, true);
        if n.raw("selected").is_some() {
            let selected = self.flag_of(n, "selected");
            a.push(attr(
                "aria-selected",
                match &selected.lit {
                    Some(v) => AttrV::S(v.string()),
                    None => AttrV::Js(format!("String({})", selected.js)),
                },
            ));
        }
        self.pressable(n, &mut a);
        // Content that is not a cell is wrapped in one so the HTML parser keeps it in the row.
        let pieces = self.pieces(n.children, &n.scope, n.depth + 1);
        let kids = self.kids(
            &pieces,
            ctx,
            Some(&mut |g: &mut Gen<'a>, leaf: Leaf<'_, 'a>| match leaf {
                Leaf::Text(text) => Some(C::J(el("td", vec![], vec![C::Text(text.to_owned())]))),
                Leaf::Node(p) if p.kind == "cell" => Some(C::J(g.cell(p, ctx))),
                Leaf::Node(p) => g.render(p, ctx).map(|c| C::J(el("td", vec![], vec![c]))),
            }),
        );
        el("tr", a, kids)
    }

    #[inline(never)]
    pub(super) fn cell(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> J<'a> {
        let a = self.base(n, true);
        el("td", a, self.content(n, ctx))
    }

    /// SPEC §5.1: one `tabs` becomes a tablist of tab buttons followed by one tabpanel per tab,
    /// of which only the selected one is shown.
    #[inline(never)]
    pub(super) fn tabs(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> C<'a> {
        let children = self.pieces(n.children, &n.scope, n.depth + 1);
        let tabs = self.collect(&children, &mut |g, leaf| {
            let Leaf::Node(t) = leaf else {
                return None;
            };
            if t.kind != "tab" {
                return None;
            }
            let panel = el("", vec![], g.content(t, ctx));
            let state = g.text_of(t, "state");
            Some(Code::of(vec![
                lit(format!(
                    "{{ id: {}, doc: {}, state: {}, label: ",
                    str_js(&t.id),
                    quote(t.doc_id),
                    state.js
                )),
                Seg::Label(t.clone()),
                lit(", panel: "),
                expr(C::J(panel)),
                lit(" }"),
            ]))
        });
        let others = filter_pieces(&children, &|leaf| match leaf {
            Leaf::Text(_) => true,
            Leaf::Node(p) => p.kind != "tab",
        });
        let other = self.kids(&others, ctx, None);
        let write = self.write(n, "selected", "_x.doc");
        let change = self.fire(n, "change");
        let choose = Self::handler(
            if self.ts { "(_k: number)" } else { "(_k)" },
            vec![
                Some("const _x = _t[_k]".to_owned()),
                Some("if (!_x || _k === _s) return".to_owned()),
                write,
                change,
            ],
        );
        let selected = self.text_of(n, "selected");
        let busy = self.use_("_busy");
        let tab_key = self.use_("_tabKey");
        let key = |keyless: bool| (!keyless).then(|| attr_js("key", "_k"));
        let keyless = self.keyless();
        let mut button_attrs: Vec<Attr<'a>> = key(keyless).into_iter().collect();
        button_attrs.extend([
            attr_js("data-weft-id", "_x.id || undefined"),
            attr_js("data-state", "_x.state || undefined"),
            attr_js("aria-busy", format!("{busy}(_x.state)")),
            attr_s("type", "button"),
            attr_s("role", "tab"),
            attr_js("id", "\"weft-\" + _x.id + \"-tab\""),
            attr_js("aria-selected", "String(_k === _s)"),
            attr_js("aria-controls", "\"weft-\" + _x.id + \"-panel\""),
            attr_js(self.an("tabIndex"), "_k === _s ? 0 : -1"),
            attr_js("onClick", "() => _choose(_k)"),
            attr_js(
                "onKeyDown",
                format!("(_e) => {tab_key}(_e, _k, _t.length, _choose)"),
            ),
        ]);
        let button = el("button", button_attrs, vec![C::Code(Code::lit("_x.label"))]);
        let mut panel_attrs: Vec<Attr<'a>> = key(keyless).into_iter().collect();
        panel_attrs.extend([
            attr_s("role", "tabpanel"),
            attr_js("id", "\"weft-\" + _x.id + \"-panel\""),
            attr_js("aria-labelledby", "\"weft-\" + _x.id + \"-tab\""),
            attr_js("hidden", "_k !== _s"),
            attr_js(self.an("tabIndex"), "0"),
        ]);
        let panel = el("div", panel_attrs, vec![C::Code(Code::lit("_x.panel"))]);
        let mut list_attrs = self.base(n, true);
        list_attrs.push(attr_s("role", "tablist"));
        let mut list_kids = vec![C::Code(Code::of(vec![
            lit("_t.map((_x, _k) => "),
            expr(C::J(button)),
            lit(")"),
        ]))];
        list_kids.extend(other);
        let tablist = el("div", list_attrs, list_kids);
        block(
            vec![
                Code::of(vec![lit("const _t = "), Seg::Code(tabs), lit(";")]),
                Code::lit(format!(
                    "const _s = Math.max(0, _t.findIndex((_x) => _x.doc === {s} || _x.id === {s}));",
                    s = selected.js
                )),
                Code::lit(format!("const _choose = {choose};")),
            ],
            C::J(el(
                "div",
                vec![attr_s("data-weft-tabs", "")],
                vec![
                    C::J(tablist),
                    C::Code(Code::of(vec![
                        lit("_t.map((_x, _k) => "),
                        expr(C::J(panel)),
                        lit(")"),
                    ])),
                ],
            )),
        )
    }

    #[inline(never)]
    pub(super) fn dialog(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        let open = self.flag_of(n, "open");
        if open.lit.as_ref().is_some_and(|v| *v != TRUE) {
            return None;
        }
        let mut a = self.base(n, true);
        a.push(attr("open", AttrV::True));
        let modal = if n.raw("modal").is_none() {
            lit_e(TRUE)
        } else {
            self.flag_of(n, "modal")
        };
        match &modal.lit {
            Some(v) => {
                if *v == TRUE {
                    a.push(attr_s("aria-modal", "true"));
                }
            }
            None => a.push(attr_js(
                "aria-modal",
                format!("{} ? \"true\" : undefined", modal.js),
            )),
        }
        let write = self.write(n, "open", "false");
        let close = self.fire(n, "close");
        a.push(attr_js(
            "onKeyDown",
            Self::handler(
                "(_e)",
                vec![
                    Some("if (_e.key !== \"Escape\") return".to_owned()),
                    Some("_e.preventDefault()".to_owned()),
                    write,
                    close,
                ],
            ),
        ));
        let dialog = C::J(el("dialog", a, self.content(n, ctx)));
        Some(match &open.lit {
            Some(_) => dialog,
            None if self.keyless() => self.cond(&open.js, dialog, None),
            None => C::Code(Code::of(vec![
                lit(format!("{} && ", open.js)),
                expr(dialog),
            ])),
        })
    }
}
