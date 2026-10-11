//! The Lit target: the element tree printed as `html` templates and wrapped in a `LitElement`.
//! The tree is the one the React and SolidJS printers get, so its attribute names are React's or
//! the DOM's, and this module translates them to Lit's binding syntax when printing.
//!
//! A template keeps the whitespace between elements, which JSX drops, so a line break is only ever
//! placed inside a tag (`</span` newline `>`), never between two tags.

use super::tree::{Attr, AttrV, C, J};
use super::{Gen, RUNTIME};
use crate::js::{is_inert, quote};

/// What the printed templates import beyond `LitElement` and `html`.
#[derive(Default)]
pub(super) struct Uses {
    pub nothing: bool,
    pub style_map: bool,
    pub static_html: bool,
}

/// HTML boolean attributes: present or absent, bound with `?name`.
const BOOLEAN: &[&str] = &["disabled", "required", "open", "selected", "hidden"];

/// Bound as DOM properties, because the attribute only sets the initial value of a control.
const PROPERTIES: &[&str] = &["value", "checked"];

/// A run of printed children: `dangling` is set while the text ends inside a tag, before its `>`.
struct Run {
    out: String,
    dangling: bool,
}

impl Run {
    /// Closes the tag the text ends in, on a line of its own before an element.
    fn close(&mut self, indent: &str, element: bool) {
        if self.dangling {
            if element {
                self.out.push('\n');
                self.out.push_str(indent);
            }
            self.out.push('>');
            self.dangling = false;
        }
    }
}

fn is_void(tag: &str) -> bool {
    matches!(tag, "img" | "input")
}

/// An expression that is never `undefined`, so it needs no `nothing` fallback.
fn always_set(code: &str) -> bool {
    code.starts_with('"') || code.starts_with("String(") || code.parse::<f64>().is_ok()
}

/// `my-screen` for `MyScreen`; a custom element name needs a hyphen.
pub(super) fn element_name(component: &str) -> String {
    let mut out = String::new();
    for (i, c) in component.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    if out.contains('-') {
        out
    } else {
        format!("weft-{out}")
    }
}

impl<'a> Gen<'a> {
    /// An element or fragment in expression position, as a template.
    pub(super) fn lit_template(&mut self, c: &C<'a>, indent: &str) -> String {
        let mut run = Run {
            out: String::new(),
            dangling: false,
        };
        self.lit_kids(std::slice::from_ref(c), &format!("{indent}  "), &mut run);
        run.close(indent, false);
        format!("html`{}`", run.out)
    }

    /// An element without its final `>`, which whatever follows supplies.
    fn lit_element(&mut self, j: &J<'a>, indent: &str) -> String {
        let mut run = Run {
            out: format!("<{}", j.tag),
            dangling: true,
        };
        for a in &j.attrs {
            let printed = self.lit_attr(a, indent);
            run.out.push_str(&printed);
        }
        if is_void(&j.tag) {
            return run.out;
        }
        let inner = format!("{indent}  ");
        let before = run.out.len();
        self.lit_kids(&j.kids, &inner, &mut run);
        let empty = run.out.len() == before;
        run.close(indent, !empty);
        format!("{}</{}", run.out, j.tag)
    }

    fn lit_kids(&mut self, kids: &[C<'a>], indent: &str, run: &mut Run) {
        for kid in kids {
            match kid {
                C::J(j) if j.tag.is_empty() => self.lit_kids(&j.kids, indent, run),
                C::J(j) => {
                    run.close(indent, true);
                    let printed = self.lit_element(j, indent);
                    run.out.push_str(&printed);
                    run.dangling = true;
                }
                C::Text(text) => {
                    run.close(indent, false);
                    run.out.push_str(&if is_inert(text) {
                        text.clone()
                    } else {
                        format!("${{{}}}", quote(text))
                    });
                }
                C::Code(code) => {
                    run.close(indent, false);
                    let printed = self.print_code(code, indent);
                    run.out.push_str(&format!("${{{printed}}}"));
                }
            }
        }
    }

    fn lit_attr(&mut self, a: &Attr<'a>, indent: &str) -> String {
        let name = a.name.as_str();
        // Templates are not diffed by key.
        if name == "key" {
            return String::new();
        }
        match &a.value {
            AttrV::True => format!(" {name}"),
            AttrV::S(text) if is_inert(text) => format!(" {name}=\"{text}\""),
            AttrV::S(text) => format!(" {name}=${{{}}}", quote(text)),
            AttrV::Js(code) => self.lit_binding(name, code),
            AttrV::Expr(c) => format!(" {name}=${{{}}}", self.expr_of(c, indent)),
        }
    }

    fn lit_binding(&mut self, name: &str, code: &str) -> String {
        if let Some(event) = name
            .strip_prefix("on")
            .filter(|e| e.starts_with(char::is_uppercase))
        {
            return format!(" @{}=${{{code}}}", event.to_ascii_lowercase());
        }
        if name == "style" {
            self.lit_uses.style_map = true;
            return format!(" style=${{styleMap({code})}}");
        }
        if BOOLEAN.contains(&name) {
            return format!(" ?{name}=${{{code}}}");
        }
        if PROPERTIES.contains(&name) {
            return format!(" .{name}=${{{code}}}");
        }
        if always_set(code) {
            format!(" {name}=${{{code}}}")
        } else {
            self.lit_uses.nothing = true;
            format!(" {name}=${{({code}) ?? nothing}}")
        }
    }

    pub(super) fn lit_module(&self, name: &str, body: &str, functions: &str) -> String {
        let uses = &self.lit_uses;
        let mut out = String::new();
        let mut names = vec!["LitElement"];
        if !uses.static_html {
            names.push("html");
        }
        if uses.nothing {
            names.push("nothing");
        }
        out.push_str(&format!(
            "import {{ {} }} from \"lit\";\n",
            names.join(", ")
        ));
        if uses.static_html {
            out.push_str("import { html, unsafeStatic } from \"lit/static-html.js\";\n");
        }
        if uses.style_map {
            out.push_str("import { styleMap } from \"lit/directives/style-map.js\";\n");
        }
        out.push('\n');
        for h in RUNTIME.iter().filter(|h| self.used.contains(h.name)) {
            out.push_str(h.js);
            out.push_str("\n\n");
        }
        if !functions.is_empty() {
            out.push_str(functions);
            out.push('\n');
        }
        let tag = element_name(name);
        out.push_str(&format!(
            "class {name} extends LitElement {{
  static properties = {{
    data: {{ attribute: false }},
    actions: {{ attribute: false }},
    onChange: {{ attribute: false }},
  }};

  // The light DOM, so the page's stylesheet and the ids that labels refer to reach the controls.
  createRenderRoot() {{
    return this;
  }}

  render() {{
    const {{ data, actions, onChange }} = this;
    return {body};
  }}
}}

if (!customElements.get(\"{tag}\")) customElements.define(\"{tag}\", {name});

export default {name};
"
        ));
        out
    }
}
