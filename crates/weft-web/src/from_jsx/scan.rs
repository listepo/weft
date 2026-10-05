//! A lexical estimate of how much stack the JSX parser will need, made before parsing. oxc descends
//! recursively, and a stack overflow aborts the process instead of returning an error, so source
//! that nests too deeply is refused here. The scanner never recurses itself.
//!
//! Each open bracket, template substitution and JSX element costs roughly the stack bytes one such
//! level took in oxc 0.152 (release build, measured by nesting each shape until a 1 MiB thread
//! overflowed: about 1.5 KB per parenthesis, 1.75 KB per object, 2.6 KB per nested function).
//! Operators that chain without brackets (`? :`, `=>`, `=`, prefix `!`, `new` …) cost less and
//! are charged to the enclosing bracket until a `,` or `;` ends the expression. The estimate is a
//! heuristic: it errs towards refusing, and the budget leaves more than half of a 1 MiB stack free.

/// Estimated stack bytes the parser may use.
const BUDGET: u32 = 400_000;

const BRACKET: u32 = 1_800;
const ELEMENT: u32 = 1_200;
const FUNCTION: u32 = 1_000;
const ARROW: u32 = 900;
const CHAIN: u32 = 600;
const KEYWORD: u32 = 300;
const PREFIX: u32 = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The top level of the module.
    Module,
    Paren,
    Square,
    Brace,
    /// A template literal's text.
    Template,
    /// `${ … }` inside a template literal.
    Substitution,
    /// Between `<name` and `>` or `/>`.
    Tag,
    /// Between `>` and the closing tag.
    Children,
    /// `{ … }` in an attribute or among children.
    Container,
}

struct Frame {
    kind: Kind,
    cost: u32,
    chain: u32,
}

struct Scanner<'t> {
    b: &'t [u8],
    i: usize,
    stack: Vec<Frame>,
    total: u32,
    /// The next token starts an operand: a `/` there begins a regular expression and a `<` an
    /// element.
    operand: bool,
    /// A line break followed what could end a statement; the next token may start a new one.
    line_break: bool,
}

/// Whether `text` is shallow enough to parse. Never panics, whatever the input.
pub(crate) fn within_budget(text: &str) -> bool {
    let mut s = Scanner {
        b: text.as_bytes(),
        i: 0,
        stack: vec![Frame {
            kind: Kind::Module,
            cost: 0,
            chain: 0,
        }],
        total: 0,
        operand: true,
        line_break: false,
    };
    s.run()
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'\\' || c >= 0x80
}

impl Scanner<'_> {
    fn peek(&self, ahead: usize) -> u8 {
        self.b.get(self.i + ahead).copied().unwrap_or(0)
    }

    fn top(&self) -> Kind {
        self.stack.last().map_or(Kind::Module, |f| f.kind)
    }

    fn push(&mut self, kind: Kind, cost: u32) -> bool {
        self.stack.push(Frame {
            kind,
            cost,
            chain: 0,
        });
        self.total = self.total.saturating_add(cost);
        self.total <= BUDGET
    }

    fn pop(&mut self) {
        if self.stack.len() > 1
            && let Some(f) = self.stack.pop()
        {
            self.total = self.total.saturating_sub(f.cost.saturating_add(f.chain));
        }
    }

    fn chain(&mut self, cost: u32) -> bool {
        if let Some(f) = self.stack.last_mut() {
            f.chain = f.chain.saturating_add(cost);
        }
        self.total = self.total.saturating_add(cost);
        self.total <= BUDGET
    }

    fn end_expression(&mut self) {
        if let Some(f) = self.stack.last_mut() {
            self.total = self.total.saturating_sub(f.chain);
            f.chain = 0;
        }
    }

    fn run(&mut self) -> bool {
        while self.i < self.b.len() {
            let ok = match self.top() {
                Kind::Template => self.template(),
                Kind::Tag => self.tag(),
                Kind::Children => self.children(),
                _ => self.code(),
            };
            if !ok {
                return false;
            }
        }
        true
    }

    fn skip_line_comment(&mut self) {
        while self.i < self.b.len() && self.b[self.i] != b'\n' {
            self.i += 1;
        }
    }

    fn skip_block_comment(&mut self) {
        self.i += 2;
        while self.i < self.b.len() && !(self.b[self.i] == b'*' && self.peek(1) == b'/') {
            if self.b[self.i] == b'\n' && !self.operand {
                self.line_break = true;
            }
            self.i += 1;
        }
        self.i += 2;
    }

    fn skip_string(&mut self, quote: u8, escapes: bool) {
        self.i += 1;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            if c == quote {
                return;
            }
            if escapes && c == b'\\' {
                self.i += 1;
            } else if escapes && c == b'\n' {
                return;
            }
        }
    }

    /// A regular expression literal. Its groups do not reach the parser (regular expressions are
    /// not parsed), but brackets inside still count while it is open, so that division mistaken
    /// for a regular expression cannot hide nesting from the estimate.
    fn regex(&mut self) -> bool {
        self.i += 1;
        let mut class = false;
        let mut depth: u32 = 0;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'\\' => self.i += 1,
                b'\n' => break,
                b'[' => {
                    class = true;
                    depth += 1;
                }
                b']' => {
                    class = false;
                    depth = depth.saturating_sub(1);
                }
                b'(' | b'{' => depth += 1,
                b')' | b'}' => depth = depth.saturating_sub(1),
                b'/' if !class => break,
                _ => {}
            }
            if self.total.saturating_add(depth.saturating_mul(BRACKET)) > BUDGET {
                return false;
            }
        }
        while self.i < self.b.len() && is_word(self.b[self.i]) {
            self.i += 1;
        }
        self.operand = false;
        true
    }

    /// After `<` in operand position: an element, unless it is a TypeScript type parameter list
    /// (`<T,>` or `<T extends U>`).
    fn starts_element(&self) -> bool {
        let first = self.peek(1);
        if first == b'>' {
            return true;
        }
        if !(first.is_ascii_alphabetic() || first == b'_' || first == b'$') {
            return false;
        }
        let mut j = self.i + 1;
        while j < self.b.len() && (is_word(self.b[j]) || self.b[j] == b'.' || self.b[j] == b'-') {
            j += 1;
        }
        while j < self.b.len() && self.b[j].is_ascii_whitespace() {
            j += 1;
        }
        let rest = &self.b[j..];
        !(rest.starts_with(b",") || rest.starts_with(b"extends ") || rest.starts_with(b"extends\n"))
    }

    fn code(&mut self) -> bool {
        let c = self.b[self.i];
        if c.is_ascii_whitespace() {
            if c == b'\n' && !self.operand {
                self.line_break = true;
            }
            self.i += 1;
            return true;
        }
        if c == b'/' && self.peek(1) == b'/' {
            self.skip_line_comment();
            return true;
        }
        if c == b'/' && self.peek(1) == b'*' {
            self.skip_block_comment();
            return true;
        }
        // A new statement after a line break (no semicolon) ends the previous chain of operators.
        if std::mem::take(&mut self.line_break)
            && (is_word(c) || c == b'"' || c == b'\'' || c == b'`')
        {
            self.end_expression();
        }
        match c {
            b'"' | b'\'' => {
                self.skip_string(c, true);
                self.operand = false;
                true
            }
            b'`' => {
                self.i += 1;
                self.push(Kind::Template, 0)
            }
            b'(' | b'[' | b'{' => {
                self.i += 1;
                self.operand = true;
                let kind = match c {
                    b'(' => Kind::Paren,
                    b'[' => Kind::Square,
                    _ => Kind::Brace,
                };
                self.push(kind, BRACKET)
            }
            b')' | b']' | b'}' => {
                self.i += 1;
                let top = self.top();
                if matches!(
                    top,
                    Kind::Paren | Kind::Square | Kind::Brace | Kind::Substitution | Kind::Container
                ) {
                    self.pop();
                }
                // After `}` a block may have ended, so an operand may follow.
                self.operand = c == b'}';
                true
            }
            b',' | b';' => {
                self.i += 1;
                self.end_expression();
                self.operand = true;
                true
            }
            b'?' => {
                if self.peek(1) == b'?' {
                    self.i += 2;
                    self.operand = true;
                    return true;
                }
                if self.peek(1) == b'.' && !self.peek(2).is_ascii_digit() {
                    self.i += 2;
                    self.operand = false;
                    return true;
                }
                self.i += 1;
                self.operand = true;
                self.chain(CHAIN)
            }
            b'=' => {
                self.operand = true;
                match self.peek(1) {
                    b'>' => {
                        self.i += 2;
                        self.chain(ARROW)
                    }
                    b'=' => {
                        self.i += 2;
                        true
                    }
                    _ => {
                        self.i += 1;
                        self.chain(CHAIN)
                    }
                }
            }
            b'!' => {
                if self.peek(1) == b'=' {
                    self.i += 2;
                    self.operand = true;
                    return true;
                }
                self.i += 1;
                // After an operand `!` is TypeScript's non-null assertion, not a prefix.
                if self.operand {
                    self.chain(PREFIX)
                } else {
                    true
                }
            }
            b'~' => {
                self.i += 1;
                self.operand = true;
                self.chain(PREFIX)
            }
            b'+' | b'-' => {
                if self.peek(1) == c {
                    self.i += 2;
                    return true;
                }
                self.i += 1;
                let prefix = self.operand;
                self.operand = true;
                if prefix { self.chain(PREFIX) } else { true }
            }
            b'*' => {
                self.operand = true;
                if self.peek(1) == b'*' {
                    self.i += 2;
                    self.chain(CHAIN)
                } else {
                    self.i += 1;
                    true
                }
            }
            b'<' => {
                if self.operand && self.starts_element() {
                    if self.peek(1) == b'>' {
                        self.i += 2;
                        self.push(Kind::Children, ELEMENT)
                    } else {
                        self.i += 1;
                        self.push(Kind::Tag, ELEMENT)
                    }
                } else {
                    self.i += 1;
                    self.operand = true;
                    true
                }
            }
            b'/' => {
                if self.operand {
                    self.regex()
                } else {
                    self.i += 1;
                    self.operand = true;
                    true
                }
            }
            b'.' => {
                if self.peek(1) == b'.' && self.peek(2) == b'.' {
                    self.i += 3;
                    self.operand = true;
                } else if self.peek(1).is_ascii_digit() {
                    self.number();
                } else {
                    self.i += 1;
                    self.operand = false;
                }
                true
            }
            _ if c.is_ascii_digit() => {
                self.number();
                true
            }
            _ if is_word(c) => self.word(),
            _ => {
                self.i += 1;
                self.operand = true;
                true
            }
        }
    }

    fn number(&mut self) {
        while self.i < self.b.len() && (is_word(self.b[self.i]) || self.b[self.i] == b'.') {
            self.i += 1;
        }
        self.operand = false;
    }

    fn word(&mut self) -> bool {
        let start = self.i;
        while self.i < self.b.len() && is_word(self.b[self.i]) {
            self.i += 1;
        }
        let word = &self.b[start..self.i];
        let (cost, operand) = match word {
            b"function" | b"class" => (FUNCTION, false),
            b"new" | b"typeof" | b"void" | b"delete" | b"await" | b"yield" => (KEYWORD, true),
            b"return" | b"case" | b"in" | b"of" | b"instanceof" | b"do" | b"else" | b"throw"
            | b"extends" => (0, true),
            _ => (0, false),
        };
        self.operand = operand;
        cost == 0 || self.chain(cost)
    }

    fn template(&mut self) -> bool {
        match self.b[self.i] {
            b'\\' => self.i += 2,
            b'`' => {
                self.i += 1;
                self.pop();
                self.operand = false;
            }
            b'$' if self.peek(1) == b'{' => {
                self.i += 2;
                self.operand = true;
                return self.push(Kind::Substitution, BRACKET);
            }
            _ => self.i += 1,
        }
        true
    }

    fn tag(&mut self) -> bool {
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                self.operand = true;
                return self.push(Kind::Container, BRACKET);
            }
            q @ (b'"' | b'\'') => self.skip_string(q, false),
            b'/' if self.peek(1) == b'>' => {
                self.i += 2;
                self.pop();
                self.operand = false;
            }
            b'>' => {
                self.i += 1;
                self.pop();
                return self.push(Kind::Children, ELEMENT);
            }
            b'<' => {
                self.i += 1;
                return self.push(Kind::Tag, ELEMENT);
            }
            _ => self.i += 1,
        }
        true
    }

    fn children(&mut self) -> bool {
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                self.operand = true;
                return self.push(Kind::Container, BRACKET);
            }
            b'<' if self.peek(1) == b'/' => {
                while self.i < self.b.len() && self.b[self.i] != b'>' {
                    self.i += 1;
                }
                self.i += 1;
                self.pop();
                self.operand = false;
            }
            b'<' if self.peek(1) == b'>' => {
                self.i += 2;
                return self.push(Kind::Children, ELEMENT);
            }
            b'<' => {
                self.i += 1;
                return self.push(Kind::Tag, ELEMENT);
            }
            _ => self.i += 1,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::within_budget;

    #[test]
    fn ordinary_components_pass() {
        let source = r#"
            import { useState } from "react";
            const re = /[(]{2}/g, half = 1 / 2;
            export default function App({ data, actions }) {
              const [open, setOpen] = useState(false)
              const label = `Hello ${data.name ? `${data.name}!` : "you"}`
              return (
                <main className="row" aria-label='Main'>
                  {data.items.map((item, i) => <li key={i}>{item.title} isn't {i < 2 ? "low" : "high"}</li>)}
                  <>
                    <input value={data.q} onChange={(e) => actions.set("q", e.target.value)} />
                  </>
                </main>
              );
            }
        "#;
        assert!(within_budget(source));
    }

    #[test]
    fn deep_nesting_of_every_shape_is_refused() {
        let shapes: [(&str, &str); 9] = [
            ("(", ")"),
            ("[", "]"),
            ("{a:", "}"),
            ("f(", ")"),
            ("(a) => (", ")"),
            ("function(){return ", "}"),
            ("<a x={", "} />"),
            ("<a>{c ? (", ") : null}</a>"),
            ("`${", "}`"),
        ];
        for (open, close) in shapes {
            let deep = format!("x = {}1{}", open.repeat(400), close.repeat(400));
            assert!(!within_budget(&deep), "{open}");
            let shallow = format!("x = {}1{}", open.repeat(20), close.repeat(20));
            assert!(within_budget(&shallow), "{open}");
        }
        for chain in ["a ? b : ", "a = ", "a => ", "!", "new ", "typeof "] {
            assert!(
                !within_budget(&format!("x = {}1", chain.repeat(5_000))),
                "{chain}"
            );
        }
    }

    #[test]
    fn brackets_inside_a_regex_still_count() {
        let deep = format!("x = /{}1{}/", "(".repeat(400), ")".repeat(400));
        assert!(!within_budget(&deep));
        assert!(within_budget("x = /((a)(b))/; y = a / (b) / c"));
    }

    #[test]
    fn statements_without_semicolons_do_not_add_up() {
        let source = "let a = 1\n".repeat(5_000);
        assert!(within_budget(&source));
    }
}
