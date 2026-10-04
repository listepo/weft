//! Syntax layer (SPEC §2, §6 layer 1): a strict tokenizer for the restricted XML subset.
//!
//! It is hand-written rather than built on an XML crate because general parsers recover silently
//! from exactly what Weft must reject and report no attribute positions, which repair-oriented
//! diagnostics need. It works on UTF-16 code units so that columns match SPEC §2 and the
//! TypeScript core.

use crate::diagnostics::{Code, Diagnostic, Position, sort_by_position};
use crate::rules::{MAX_DEPTH, is_name, is_non_xml_char};
use crate::source::path_segment;

pub struct RawAttribute {
    pub name: String,
    pub value: String,
    pub pos: Position,
}

pub enum RawChild {
    /// Index into [`SyntaxResult::elements`].
    Element(usize),
    /// Text as written after reference decoding; whitespace is normalized later by the builder.
    Text { text: String, pos: Position },
}

pub struct RawElement {
    pub name: String,
    pub attrs: Vec<RawAttribute>,
    pub children: Vec<RawChild>,
    pub pos: Position,
}

impl RawElement {
    pub fn attr(&self, name: &str) -> Option<&RawAttribute> {
        self.attrs.iter().find(|a| a.name == name)
    }

    pub fn segment(&self) -> String {
        path_segment(&self.name, self.attr("id").map(|a| a.value.as_str()), None)
    }
}

pub struct SyntaxResult {
    /// Every element read, including ones that are not reachable from `root`.
    pub elements: Vec<RawElement>,
    pub root: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
}

const PREDEFINED: [(&str, &str); 5] = [
    ("lt", "<"),
    ("gt", ">"),
    ("amp", "&"),
    ("quot", "\""),
    ("apos", "'"),
];
const MAX_CODE_POINT: u32 = 0x10FFFF;
const DECIMAL: u32 = 10;
const HEX: u32 = 16;
const BOM: char = '\u{FEFF}';

/// ECMAScript `\s`, which the TypeScript tokenizer uses for names and unquoted values.
fn is_js_space(u: u16) -> bool {
    matches!(
        u,
        0x09..=0x0D
            | 0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

/// XML whitespace after end-of-line handling.
fn is_space(u: u16) -> bool {
    matches!(u, 0x09 | 0x0A | 0x20)
}

fn is_name_unit(u: u16) -> bool {
    !is_js_space(u) && !matches!(u, 0x2F | 0x3E | 0x3D | 0x3C | 0x22 | 0x27)
}

fn unit(c: char) -> u16 {
    // Only ASCII is compared against, so the cast cannot truncate.
    c as u16
}

fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

struct Tokenizer {
    src: Vec<u16>,
    line_starts: Vec<usize>,
    diagnostics: Vec<Diagnostic>,
    elements: Vec<RawElement>,
    stack: Vec<usize>,
    root: Option<usize>,
    text: Vec<u16>,
    text_start: Option<usize>,
}

struct Report<'a> {
    message: String,
    expected: &'a str,
    got: Option<String>,
    hint: Option<String>,
}

fn report(message: impl Into<String>, expected: &str) -> Report<'_> {
    Report {
        message: message.into(),
        expected,
        got: None,
        hint: None,
    }
}

impl Report<'_> {
    fn got(mut self, got: impl Into<String>) -> Self {
        self.got = Some(got.into());
        self
    }

    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl Tokenizer {
    fn len(&self) -> usize {
        self.src.len()
    }

    fn at(&self, k: usize) -> Option<u16> {
        self.src.get(k).copied()
    }

    fn is(&self, k: usize, c: char) -> bool {
        self.at(k) == Some(unit(c))
    }

    fn starts_with(&self, k: usize, s: &str) -> bool {
        let pattern: Vec<u16> = s.encode_utf16().collect();
        self.src.get(k..k + pattern.len()) == Some(&pattern[..])
    }

    fn find(&self, from: usize, s: &str) -> Option<usize> {
        let pattern: Vec<u16> = s.encode_utf16().collect();
        let from = from.min(self.len());
        self.src[from..]
            .windows(pattern.len())
            .position(|w| w == pattern)
            .map(|i| i + from)
    }

    fn pos_at(&self, offset: usize) -> Position {
        let line = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .max(1)
            - 1;
        let start = self.line_starts.get(line).copied().unwrap_or(0);
        Position {
            line: to_u32(line + 1),
            column: to_u32(offset - start + 1),
        }
    }

    fn path_of(&self, extra: Option<&str>) -> String {
        let mut segments: Vec<String> = self
            .stack
            .iter()
            .map(|&e| self.elements[e].segment())
            .collect();
        if let Some(extra) = extra {
            segments.push(extra.to_owned());
        }
        format!("/{}", segments.join("/"))
    }

    fn report(&mut self, code: Code, offset: usize, r: Report<'_>, extra: Option<&str>) {
        let d = Diagnostic::new(code, self.path_of(extra), r.message, r.expected)
            .pos(Some(self.pos_at(offset)))
            .got_opt(r.got)
            .hint_opt(r.hint);
        self.diagnostics.push(d);
    }

    /// A reference that cannot be decoded is kept verbatim so that tokenizing continues.
    fn read_reference(&mut self, at: usize) -> (Vec<u16>, usize) {
        let Some((kind, body, end)) = self.match_reference(at) else {
            self.report(
                Code::W112,
                at,
                report(
                    "A bare `&` must start an entity or character reference.",
                    "&lt; &gt; &amp; &quot; &apos; or &#N;",
                )
                .hint("write \"&amp;\" for a literal ampersand"),
                None,
            );
            return (vec![unit('&')], at + 1);
        };
        let whole = text(&self.src[at..end]);
        let name = text(body);
        if kind == RefKind::Named {
            if let Some((_, value)) = PREDEFINED.iter().find(|(n, _)| *n == name) {
                return (value.encode_utf16().collect(), end);
            }
            self.report(
                Code::W112,
                at,
                report(
                    format!("Entity \"&{name};\" is not one of the five predefined XML entities."),
                    "&lt; &gt; &amp; &quot; &apos; or a numeric reference",
                )
                .got(whole.clone())
                .hint("use the character itself (the file is UTF-8) or a numeric reference such as \"&#160;\""),
                None,
            );
            return (whole.encode_utf16().collect(), end);
        }
        let radix = if kind == RefKind::Decimal {
            DECIMAL
        } else {
            HEX
        };
        // Overflow means "too large", which is rejected like any code point above U+10FFFF.
        let code = u32::from_str_radix(&name, radix).unwrap_or(u32::MAX);
        let char = (code <= MAX_CODE_POINT)
            .then(|| char::from_u32(code))
            .flatten();
        match char {
            Some(c) if !is_non_xml_char(c) => {
                let mut buf = [0u16; 2];
                (c.encode_utf16(&mut buf).to_vec(), end)
            }
            _ => {
                self.report(
                    Code::W112,
                    at,
                    report(
                        format!(
                            "Character reference \"{whole}\" names a character XML does not allow."
                        ),
                        "a reference to a character XML allows",
                    )
                    .got(whole.clone()),
                    None,
                );
                (whole.encode_utf16().collect(), end)
            }
        }
    }

    /// `&(?:([A-Za-z][A-Za-z0-9]*)|#([0-9]+)|#x([0-9A-Fa-f]+));` anchored at `at`.
    fn match_reference(&self, at: usize) -> Option<(RefKind, &[u16], usize)> {
        let ascii = |k: usize, f: fn(&u8) -> bool| {
            self.at(k)
                .is_some_and(|u| u8::try_from(u).is_ok_and(|b| f(&b)))
        };
        let run = |from: usize, f: fn(&u8) -> bool| {
            let mut k = from;
            while ascii(k, f) {
                k += 1;
            }
            k
        };
        let finish = |kind, from: usize, to: usize| {
            (to > from && self.is(to, ';')).then(|| (kind, &self.src[from..to], to + 1))
        };
        let k = at + 1;
        if ascii(k, u8::is_ascii_alphabetic) {
            return finish(RefKind::Named, k, run(k, u8::is_ascii_alphanumeric));
        }
        if !self.is(k, '#') {
            return None;
        }
        finish(RefKind::Decimal, k + 1, run(k + 1, u8::is_ascii_digit)).or_else(|| {
            if self.is(k + 1, 'x') {
                finish(RefKind::Hex, k + 2, run(k + 2, u8::is_ascii_hexdigit))
            } else {
                None
            }
        })
    }

    fn flush_text(&mut self) {
        if let Some(start) = self.text_start.take() {
            match self.stack.last().copied() {
                None => self.report(
                    Code::W114,
                    start,
                    report(
                        "Text outside the root element.",
                        "only comments and whitespace around the root",
                    )
                    .hint("move the text inside an element such as <text>"),
                    None,
                ),
                Some(parent) => {
                    let child = RawChild::Text {
                        text: text(&self.text),
                        pos: self.pos_at(start),
                    };
                    self.elements[parent].children.push(child);
                }
            }
        }
        self.text.clear();
    }

    fn append_text(&mut self, chunk: &[u16], at: usize) {
        if self.text_start.is_none()
            && let Some(first) = chunk.iter().position(|&u| !is_space(u))
        {
            self.text_start = Some(at + first);
        }
        self.text.extend_from_slice(chunk);
    }

    fn read_name(&self, at: usize) -> String {
        let mut k = at;
        while self.at(k).is_some_and(is_name_unit) {
            k += 1;
        }
        text(&self.src[at.min(k)..k])
    }

    fn name_len(name: &str) -> usize {
        name.encode_utf16().count()
    }

    fn skip_whitespace(&self, at: usize) -> usize {
        let mut k = at;
        while self.at(k).is_some_and(is_space) {
            k += 1;
        }
        k
    }

    fn check_name(&mut self, name: &str, at: usize, what: &str, extra: Option<&str>) {
        if is_name(name) {
            return;
        }
        let suggestion = suggest_name(name);
        let mut r = report(
            format!("{what} name \"{name}\" breaks the name grammar."),
            "[a-z][a-z0-9]*(-[a-z0-9]+)*",
        )
        .got(name);
        if is_name(&suggestion) {
            r = r.hint(format!("did you mean \"{suggestion}\"?"));
        }
        self.report(Code::W105, at, r, extra);
    }

    fn read_attribute_value(
        &mut self,
        at: usize,
        quote: Option<u16>,
        name: &str,
    ) -> (String, usize) {
        let extra = format!("@{name}");
        let mut value: Vec<u16> = Vec::new();
        let mut k = at;
        loop {
            let Some(c) = self.at(k) else {
                self.report(
                    Code::W110,
                    at,
                    report(
                        format!("Value of attribute \"{name}\" is never closed."),
                        "a closing \"",
                    )
                    .hint("close it with \""),
                    Some(&extra),
                );
                return (text(&value), k);
            };
            let ends = match quote {
                None => is_js_space(c) || c == unit('>') || self.starts_with(k, "/>"),
                Some(q) => c == q,
            };
            if ends {
                return (text(&value), if quote.is_none() { k } else { k + 1 });
            }
            if c == unit('<') {
                self.report(
                    Code::W113,
                    k,
                    report("`<` is not allowed inside an attribute value.", "&lt;")
                        .got("<")
                        .hint("write \"&lt;\""),
                    Some(&extra),
                );
                value.push(c);
                k += 1;
            } else if c == unit('&') {
                let (decoded, end) = self.read_reference(k);
                value.extend(decoded);
                k = end;
            } else {
                // XML attribute-value normalization: literal tabs and newlines read as spaces.
                value.push(if c == unit('\t') || c == unit('\n') {
                    unit(' ')
                } else {
                    c
                });
                k += 1;
            }
        }
    }

    /// Returns the offset after the start tag, or `None` when parsing must stop.
    fn read_start_tag(&mut self, at: usize) -> Option<usize> {
        let name = self.read_name(at + 1);
        if name.is_empty() {
            self.report(
                Code::W101,
                at,
                report("`<` must start a tag.", "an element name after <")
                    .hint("write \"&lt;\" for a literal \"<\""),
                None,
            );
            self.append_text(&[unit('<')], at);
            return Some(at + 1);
        }
        let index = self.elements.len();
        self.elements.push(RawElement {
            name: name.clone(),
            attrs: vec![],
            children: vec![],
            pos: self.pos_at(at),
        });
        match (self.stack.last().copied(), self.root) {
            (Some(parent), _) => self.elements[parent]
                .children
                .push(RawChild::Element(index)),
            (None, None) => self.root = Some(index),
            (None, Some(_)) => self.report(
                Code::W114,
                at,
                report(
                    "A document has exactly one root element.",
                    "one root element",
                )
                .got(format!("<{name}>")),
                None,
            ),
        }
        if self.stack.len() >= MAX_DEPTH {
            let expected = format!("at most {MAX_DEPTH}");
            self.report(
                Code::W117,
                at,
                report(
                    format!("Elements nest deeper than {MAX_DEPTH} levels."),
                    &expected,
                ),
                None,
            );
            return None;
        }
        // The element is open while its attributes are read, so that their paths include it.
        self.stack.push(index);
        self.check_name(&name, at + 1, "Element", None);

        let n = self.len();
        let mut k = at + 1 + Self::name_len(&name);
        let mut seen: Vec<String> = Vec::new();
        loop {
            let before = k;
            k = self.skip_whitespace(k);
            if k >= n {
                self.report(
                    Code::W110,
                    at,
                    report(
                        format!("Start tag <{name}> is never closed."),
                        "\">\" or \"/>\"",
                    )
                    .hint("end it with \">\" or \"/>\""),
                    None,
                );
                self.stack.pop();
                return Some(n);
            }
            if self.starts_with(k, "/>") {
                self.stack.pop();
                return Some(k + 2);
            }
            if self.is(k, '>') {
                return Some(k + 1);
            }
            let attr_name = self.read_name(k);
            if attr_name.is_empty() {
                let c = text(&self.src[k..k + 1]);
                self.report(
                    Code::W101,
                    k,
                    report(
                        format!("Unexpected \"{c}\" inside <{name}>."),
                        "an attribute, \">\" or \"/>\"",
                    )
                    .got(c),
                    None,
                );
                k += 1;
                continue;
            }
            if k == before {
                self.report(
                    Code::W101,
                    k,
                    report("Attributes must be separated by whitespace.", "whitespace")
                        .hint(format!("add a space before {attr_name}")),
                    None,
                );
            }
            let attr_at = k;
            let extra = format!("@{attr_name}");
            self.check_name(&attr_name, attr_at, "Attribute", Some(&extra));
            k = self.skip_whitespace(k + Self::name_len(&attr_name));
            let mut value = String::new();
            if !self.is(k, '=') {
                let expected = format!("{attr_name}=\"…\"");
                self.report(
                    Code::W107,
                    attr_at,
                    report(
                        format!("Attribute \"{attr_name}\" has no value."),
                        &expected,
                    )
                    .hint(format!("write {attr_name}=\"true\"")),
                    Some(&extra),
                );
            } else {
                k = self.skip_whitespace(k + 1);
                let q = self.at(k);
                if q != Some(unit('"')) {
                    let got = if q == Some(unit('\'')) {
                        "single quotes"
                    } else {
                        "no quotes"
                    };
                    self.report(
                        Code::W106,
                        k,
                        report(
                            format!("Value of attribute \"{attr_name}\" must be in double quotes."),
                            "double quotes",
                        )
                        .got(got)
                        .hint(format!("write {attr_name}=\"…\"")),
                        Some(&extra),
                    );
                }
                let quote = q.filter(|&q| q == unit('"') || q == unit('\''));
                let start = if quote.is_some() { k + 1 } else { k };
                let (read, end) = self.read_attribute_value(start, quote, &attr_name);
                value = read;
                k = end;
            }
            if seen.contains(&attr_name) {
                self.report(
                    Code::W108,
                    attr_at,
                    report(
                        format!("Attribute \"{attr_name}\" appears twice."),
                        "each attribute once",
                    )
                    .hint("keep one of them"),
                    Some(&extra),
                );
            } else {
                let pos = self.pos_at(attr_at);
                seen.push(attr_name.clone());
                self.elements[index].attrs.push(RawAttribute {
                    name: attr_name,
                    value,
                    pos,
                });
            }
        }
    }

    fn read_end_tag(&mut self, at: usize) -> usize {
        let name = self.read_name(at + 2);
        let mut k = self.skip_whitespace(at + 2 + Self::name_len(&name));
        if self.is(k, '>') {
            k += 1;
        } else {
            self.report(
                Code::W101,
                k,
                report(format!("Closing tag </{name}> must end with \">\"."), ">"),
                None,
            );
            k = self.find(k, ">").map_or(self.len(), |close| close + 1);
        }
        let Some(top) = self.stack.last().copied() else {
            self.report(
                Code::W111,
                at,
                report(
                    format!("Closing tag </{name}> has no open element."),
                    "no closing tag",
                )
                .got(format!("</{name}>"))
                .hint("remove it"),
                None,
            );
            return k;
        };
        let top_name = self.elements[top].name.clone();
        if top_name == name {
            self.stack.pop();
            return k;
        }
        let expected = format!("</{top_name}>");
        self.report(
            Code::W109,
            at,
            report(
                format!("Closing tag </{name}> does not match <{top_name}>."),
                &expected,
            )
            .got(format!("</{name}>"))
            .hint(format!("close <{top_name}> first")),
            None,
        );
        // Recover by closing up to a matching open element, if there is one.
        if let Some(found) = self
            .stack
            .iter()
            .rposition(|&e| self.elements[e].name == name)
        {
            self.stack.truncate(found);
        }
        k
    }

    fn report_non_xml_chars(&mut self) {
        let mut offset = 0;
        for c in text(&self.src).chars() {
            if is_non_xml_char(c) {
                let code = format!("{:04X}", u32::from(c));
                self.report(
                    Code::W113,
                    offset,
                    report(
                        format!("Character U+{code} is not allowed in XML."),
                        "a character XML allows",
                    )
                    .got(format!("U+{code}"))
                    .hint("remove it"),
                    None,
                );
            }
            offset += c.len_utf16();
        }
    }

    fn run(&mut self) -> bool {
        self.report_non_xml_chars();
        let n = self.len();
        let mut i = 0;
        while i < n {
            if self.is(i, '<') {
                if self.starts_with(i, "<!--") {
                    // Comments are not part of the model, so text on both sides forms one run.
                    let Some(end) = self.find(i + 4, "-->") else {
                        self.report(
                            Code::W115,
                            i,
                            report("Comment is never closed.", "-->").hint("end it with \"-->\""),
                            None,
                        );
                        i = n;
                        continue;
                    };
                    let body = &self.src[i + 4..end];
                    let dash = unit('-');
                    if body.windows(2).any(|w| w == [dash, dash]) || body.last() == Some(&dash) {
                        self.report(
                            Code::W115,
                            i,
                            report(
                                "A comment must not contain \"--\" or end with \"-\".",
                                "a comment without \"--\"",
                            ),
                            None,
                        );
                    }
                    i = end + 3;
                    continue;
                }
                self.flush_text();
                if self.starts_with(i, "<![CDATA[") {
                    self.report(
                        Code::W104,
                        i,
                        report("CDATA sections are not allowed.", "escaped text")
                            .hint("write the text with &lt; and &amp; instead"),
                        None,
                    );
                    i = self.find(i, "]]>").map_or(n, |end| end + 3);
                } else if self.starts_with(i, "<!") {
                    self.report(
                        Code::W103,
                        i,
                        report(
                            "DOCTYPE and other declarations are not allowed.",
                            "no declaration",
                        )
                        .hint("remove it"),
                        None,
                    );
                    let mut depth = 0i64;
                    let mut k = i + 2;
                    while k < n {
                        match self.at(k) {
                            Some(0x5B) => depth += 1,
                            Some(0x5D) => depth -= 1,
                            Some(0x3E) if depth <= 0 => break,
                            _ => {}
                        }
                        k += 1;
                    }
                    i = k + 1;
                } else if self.starts_with(i, "<?") {
                    self.report(
                        Code::W102,
                        i,
                        report(
                            "XML declarations and processing instructions are not allowed.",
                            "no declaration",
                        )
                        .hint("remove it"),
                        None,
                    );
                    i = self.find(i, "?>").map_or(n, |end| end + 2);
                } else if self.starts_with(i, "</") {
                    i = self.read_end_tag(i);
                } else {
                    match self.read_start_tag(i) {
                        Some(next) => i = next,
                        None => return false,
                    }
                }
                continue;
            }
            if self.is(i, '&') {
                let (value, end) = self.read_reference(i);
                self.append_text(&value, i);
                i = end;
                continue;
            }
            let mut end = i;
            while end < n && !self.is(end, '<') && !self.is(end, '&') {
                end += 1;
            }
            let chunk: Vec<u16> = self.src[i..end].to_vec();
            let cdata_end: Vec<u16> = "]]>".encode_utf16().collect();
            if let Some(at) = chunk.windows(cdata_end.len()).position(|w| w == cdata_end) {
                self.report(
                    Code::W113,
                    i + at,
                    report("\"]]>\" is not allowed in text.", "]]&gt;").hint("write \"]]&gt;\""),
                    None,
                );
            }
            self.append_text(&chunk, i);
            i = end;
        }
        self.flush_text();
        while let Some(&top) = self.stack.last() {
            let el = &self.elements[top];
            let expected = format!("</{}>", el.name);
            let d = Diagnostic::new(
                Code::W110,
                self.path_of(None),
                format!("<{}> is never closed.", el.name),
                expected,
            )
            .pos(Some(el.pos))
            .hint(format!("add </{}>", el.name));
            self.diagnostics.push(d);
            self.stack.pop();
        }
        if self.root.is_none() {
            let d = Diagnostic::new(
                Code::W114,
                "/",
                "The document has no root element.",
                "<screen id=\"…\" weft=\"0.1\">",
            )
            .pos(Some(self.pos_at(n)));
            self.diagnostics.push(d);
        }
        true
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Named,
    Decimal,
    Hex,
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// camelCase → kebab-case, lowercased, with `_`, `:` and `.` runs turned into hyphens.
fn suggest_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if (c.is_ascii_lowercase() || c.is_ascii_digit())
            && next.is_some_and(|n| n.is_ascii_uppercase())
        {
            out.push(c);
            out.push('-');
            out.extend(next);
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    let mut result = String::new();
    let mut in_run = false;
    for c in out.to_lowercase().chars() {
        if matches!(c, '_' | ':' | '.') {
            if !in_run {
                result.push('-');
            }
            in_run = true;
        } else {
            result.push(c);
            in_run = false;
        }
    }
    result
}

pub fn tokenize(input: &str) -> SyntaxResult {
    // XML end-of-line handling, so that positions and values agree with any conforming parser.
    let src = input
        .strip_prefix(BOM)
        .unwrap_or(input)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let src: Vec<u16> = src.encode_utf16().collect();
    let mut line_starts = vec![0];
    line_starts.extend(
        src.iter()
            .enumerate()
            .filter(|(_, u)| **u == unit('\n'))
            .map(|(k, _)| k + 1),
    );
    let mut t = Tokenizer {
        src,
        line_starts,
        diagnostics: vec![],
        elements: vec![],
        stack: vec![],
        root: None,
        text: vec![],
        text_start: None,
    };
    let complete = t.run();
    sort_by_position(&mut t.diagnostics);
    SyntaxResult {
        elements: t.elements,
        root: if complete { t.root } else { None },
        diagnostics: t.diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(input: &str) -> Vec<&'static str> {
        tokenize(input)
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect()
    }

    #[test]
    fn well_formed_markup_has_no_diagnostics() {
        let r = tokenize("\u{FEFF}<a x=\"1 &amp; &#x41;\">t<!-- c --><b/></a>\r\n");
        assert!(r.diagnostics.is_empty());
        let root = r.root.map(|i| &r.elements[i]);
        assert_eq!(root.map(|e| e.attrs[0].value.as_str()), Some("1 & A"));
        assert_eq!(root.map(|e| e.children.len()), Some(2));
    }

    #[test]
    fn columns_count_utf16_code_units() {
        let r = tokenize("<a>\u{1F600}&bad;</a>");
        let d = &r.diagnostics[0];
        assert_eq!((d.code, d.line, d.column), (Code::W112, Some(1), Some(6)));
    }

    #[test]
    fn each_syntax_error_has_its_code() {
        assert_eq!(codes("<?xml version=\"1.0\"?><a/>"), ["W102"]);
        assert_eq!(codes("<!DOCTYPE a [<!x>]><a/>"), ["W103"]);
        assert_eq!(codes("<a><![CDATA[x]]></a>"), ["W104"]);
        assert_eq!(codes("<myEl/>"), ["W105"]);
        assert_eq!(codes("<a x='1'/>"), ["W106"]);
        assert_eq!(codes("<a x/>"), ["W107"]);
        assert_eq!(codes("<a x=\"1\" x=\"2\"/>"), ["W108"]);
        assert_eq!(codes("<a><b></a>"), ["W109"]);
        assert_eq!(codes("<a>"), ["W110"]);
        assert_eq!(codes("<a/></a>"), ["W111"]);
        assert_eq!(codes("<a>&</a>"), ["W112"]);
        assert_eq!(codes("<a>&#0;</a>"), ["W112"]);
        assert_eq!(codes("<a>]]></a>"), ["W113"]);
        assert_eq!(codes("<a/><b/>"), ["W114"]);
        assert_eq!(codes(""), ["W114"]);
        assert_eq!(codes("<a><!-- a -- b --></a>"), ["W115"]);
    }

    #[test]
    fn deep_nesting_stops_without_a_root() {
        let r = tokenize(&"<a>".repeat(MAX_DEPTH + 1));
        assert!(r.root.is_none());
        assert_eq!(
            r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
            [Code::W117]
        );
    }

    #[test]
    fn name_suggestions_follow_kebab_case() {
        assert_eq!(suggest_name("myEl"), "my-el");
        assert_eq!(suggest_name("a_b.c"), "a-b-c");
        assert_eq!(suggest_name("aBC"), "a-bc");
    }
}
