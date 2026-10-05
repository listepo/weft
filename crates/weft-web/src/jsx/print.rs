//! Printing the element tree as JSX source. Printing needs the generator: a few expressions are
//! resolved only when printed (see `Seg::Label`).

use super::Gen;
use super::tree::{Attr, AttrV, C, Code, J, Seg};
use crate::js::{is_inert, is_inert_text, quote};

impl<'a> Gen<'a> {
    #[inline(never)]
    fn print_attr(&mut self, a: &Attr<'a>, indent: &str) -> String {
        let name = &a.name;
        match &a.value {
            AttrV::True => format!(" {name}"),
            AttrV::S(text) if is_inert(text) => format!(" {name}=\"{text}\""),
            AttrV::S(text) => format!(" {name}={{{}}}", quote(text)),
            AttrV::Js(code) => format!(" {name}={{{code}}}"),
            AttrV::Expr(c) => format!(" {name}={{{}}}", self.expr_of(c, indent)),
        }
    }

    fn print_text(text: &str) -> String {
        if is_inert(text) && is_inert_text(text) {
            text.to_owned()
        } else {
            format!("{{{}}}", quote(text))
        }
    }

    // One frame per element level; kept out of its callers so their frames stay small.
    #[inline(never)]
    pub(super) fn print_j(&mut self, j: &J<'a>, indent: &str) -> String {
        let mut open = format!("<{}", j.tag);
        for a in &j.attrs {
            let printed = self.print_attr(a, indent);
            open.push_str(&printed);
        }
        let close = if j.tag.is_empty() {
            "</>".to_owned()
        } else {
            format!("</{}>", j.tag)
        };
        if j.kids.is_empty() {
            return if j.tag.is_empty() {
                "<></>".to_owned()
            } else {
                format!("{open} />")
            };
        }
        if let [C::Text(text)] = j.kids.as_slice() {
            return format!("{open}>{}{close}", Self::print_text(text));
        }
        let inner = format!("{indent}  ");
        let mut body = Vec::with_capacity(j.kids.len());
        for k in &j.kids {
            let printed = self.print_c(k, &inner);
            body.push(format!("{inner}{printed}"));
        }
        format!("{open}>\n{}\n{indent}{close}", body.join("\n"))
    }

    #[inline(never)]
    fn print_c(&mut self, c: &C<'a>, indent: &str) -> String {
        match c {
            C::J(j) => self.print_j(j, indent),
            C::Text(text) => Self::print_text(text),
            C::Code(code) => format!("{{{}}}", self.print_code(code, indent)),
        }
    }

    /// An element or expression in expression position.
    #[inline(never)]
    pub(super) fn expr_of(&mut self, c: &C<'a>, indent: &str) -> String {
        if self.lit() && matches!(c, C::J(_)) {
            return self.lit_template(c, indent);
        }
        match c {
            C::J(j) => {
                let inner = format!("{indent}  ");
                let printed = self.print_j(j, &inner);
                format!("(\n{inner}{printed}\n{indent})")
            }
            C::Text(text) => quote(text),
            C::Code(code) => self.print_code(code, indent),
        }
    }

    #[inline(never)]
    pub(super) fn print_code(&mut self, code: &Code<'a>, indent: &str) -> String {
        let mut out = String::new();
        for seg in &code.0 {
            match seg {
                Seg::Lit(text) => out.push_str(text),
                Seg::Expr(c) => {
                    let printed = self.expr_of(c, indent);
                    out.push_str(&printed);
                }
                Seg::Code(inner) => {
                    let printed = self.print_code(inner, indent);
                    out.push_str(&printed);
                }
                Seg::Array(items) => {
                    if items.is_empty() {
                        out.push_str("[]");
                        continue;
                    }
                    let inner = format!("{indent}  ");
                    out.push_str("[\n");
                    let mut lines = Vec::with_capacity(items.len());
                    for item in items {
                        let printed = self.print_code(item, &inner);
                        lines.push(format!("{inner}{printed},"));
                    }
                    out.push_str(&lines.join("\n"));
                    out.push('\n');
                    out.push_str(indent);
                    out.push(']');
                }
                Seg::Block(lines, result) => {
                    let inner = format!("{indent}  ");
                    out.push_str("(() => {\n");
                    for line in lines {
                        let printed = self.print_code(line, &inner);
                        out.push_str(&format!("{inner}{printed}\n"));
                    }
                    let printed = self.expr_of(result, &inner);
                    out.push_str(&format!("{inner}return {printed};\n{indent}}})()"));
                }
                Seg::Label(n) => {
                    let label = self.label(n);
                    out.push_str(&label.js);
                }
                Seg::Value(n) => {
                    let value = self.prop(n, "value").0;
                    out.push_str(&value.js);
                }
            }
        }
        out
    }
}
