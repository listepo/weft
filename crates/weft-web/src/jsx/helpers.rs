//! Free helpers of the generator and the TypeScript types it can print.

use serde_json::Value as Json;
use weft_core::is_token;

use super::model::N;
use super::tree::{C, Code, J, Seg, el};

pub(super) const TS_TYPES: &str =
    "export type WeftEvent = { id: string; action: string; item?: string };

export type WeftProps = {
  data?: any;
  actions?: Record<string, (event: WeftEvent) => void>;
  onChange?: (path: string, value: unknown) => void;
};
";

pub(super) const TS_KEY: &str = "
type WeftKey = {
  key: string;
  target: unknown;
  currentTarget: { parentElement: HTMLElement | null };
  preventDefault(): void;
};
";

/// `price-row` → `PriceRow`, a component name the JSX targets accept.
pub(super) fn component_name(name: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in name.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        "Fragment".to_owned()
    } else {
        out
    }
}

pub(super) fn grows(n: &N<'_>) -> bool {
    matches!(n.raw("grow"), Some(Json::Bool(true)))
        || matches!(n.raw("grow"), Some(Json::String(s)) if s == "true")
}

/// A dimension token as `var(--weft-…)`, or nothing when the prop is not a token.
pub(super) fn css_var(n: &N<'_>, name: &str) -> Option<String> {
    n.raw(name)
        .and_then(Json::as_object)
        .and_then(|g| g.get("token"))
        .and_then(Json::as_str)
        .filter(|t| is_token(t))
        .and_then(crate::html::token_var)
        .map(|var| format!("var({var})"))
}

pub(super) fn justify_css(value: &str) -> Option<&'static str> {
    match value {
        "center" => Some("center"),
        "end" => Some("flex-end"),
        "space-between" => Some("space-between"),
        _ => None,
    }
}

pub(super) fn outermost<'a, 'b>(out: &'b mut C<'a>) -> Option<&'b mut J<'a>> {
    match out {
        C::J(j) => Some(j),
        C::Code(code) => code.0.iter_mut().find_map(|seg| match seg {
            Seg::Block(_, result) => outermost(result),
            _ => None,
        }),
        C::Text(_) => None,
    }
}

/// `(() => { const …; return <…>; })()`, for elements that need values computed once.
pub(super) fn block<'a>(lines: Vec<Code<'a>>, result: C<'a>) -> C<'a> {
    C::Code(Code::of(vec![Seg::Block(lines, Box::new(result))]))
}

pub(super) fn cell_wrap<'a>(c: Option<C<'a>>) -> Option<C<'a>> {
    c.map(|c| C::J(el("tr", vec![], vec![C::J(el("td", vec![], vec![c]))])))
}
