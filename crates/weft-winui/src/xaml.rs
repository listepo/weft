//! The XAML the generator writes, as a small element tree printed with fixed indentation. It is
//! the trust boundary of the generator: every attribute value is escaped as XML here, and
//! document text becomes a value only through `literal`.

use std::borrow::Cow;

use weft_core::escape_attribute;

const INDENT: &str = "    ";

pub struct Element {
    pub name: Cow<'static, str>,
    attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// The content of an element without children, such as a resource's value.
    text: Option<String>,
}

impl Element {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Element {
            name: name.into(),
            attrs: vec![],
            children: vec![],
            text: None,
        }
    }

    pub fn text(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
    }

    pub fn attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attrs.push((name.into(), value.into()));
    }

    /// Removes an attribute already set and returns its value, for a wrapper to carry instead.
    pub fn take(&mut self, name: &str) -> Option<String> {
        let at = self.attrs.iter().position(|(n, _)| n == name)?;
        Some(self.attrs.remove(at).1)
    }

    /// The value of an attribute already set.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn write(&self, depth: usize, out: &mut String) {
        out.push_str(&INDENT.repeat(depth));
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attrs {
            out.push_str(&format!(" {name}=\"{}\"", escape_attribute(value)));
        }
        if let Some(text) = self.text.as_deref().filter(|_| self.children.is_empty()) {
            let name = &self.name;
            out.push_str(&format!(">{}</{name}>\n", escape_attribute(text)));
            return;
        }
        if self.children.is_empty() {
            out.push_str(" />\n");
            return;
        }
        out.push_str(">\n");
        for child in &self.children {
            child.write(depth + 1, out);
        }
        out.push_str(&format!("{}</{}>\n", INDENT.repeat(depth), self.name));
    }
}

/// Document text as an attribute value. XAML reads a value that starts with `{` as a markup
/// extension, so such text gets the `{}` escape sequence in front, which XAML drops.
pub fn literal(text: &str) -> String {
    if text.starts_with('{') {
        format!("{{}}{text}")
    } else {
        text.to_owned()
    }
}

pub fn boolean(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// `login-form` → `LoginForm`, the rule of the Slint target's component names: a C# type name
/// part. A name that would start with a digit, or be empty, gets a `Weft` prefix.
pub fn type_name(id: &str) -> String {
    let mut out = String::new();
    for part in id.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out.insert_str(0, "Weft");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_cannot_open_a_markup_extension_or_break_the_attribute() {
        assert_eq!(literal("{x:Bind Secret}"), "{}{x:Bind Secret}");
        assert_eq!(literal("a {b}"), "a {b}");
        let mut e = Element::new("TextBlock");
        e.attr("Text", literal("\"><x a='1'>&\n"));
        let mut out = String::new();
        e.write(0, &mut out);
        assert_eq!(
            out,
            "<TextBlock Text=\"&quot;>&lt;x a='1'>&amp;&#10;\" />\n"
        );
    }

    #[test]
    fn text_content_is_escaped() {
        let mut e = Element::new("x:Double");
        e.attr("x:Key", "Weft.space.md");
        e.text("<12&");
        let mut out = String::new();
        e.write(1, &mut out);
        assert_eq!(
            out,
            "    <x:Double x:Key=\"Weft.space.md\">&lt;12&amp;</x:Double>\n"
        );
    }

    #[test]
    fn names_become_type_names() {
        assert_eq!(type_name("login-form"), "LoginForm");
        assert_eq!(type_name("2fa"), "Weft2fa");
        assert_eq!(type_name("--"), "Weft");
        assert_eq!(type_name("SignIn"), "SignIn");
    }
}
