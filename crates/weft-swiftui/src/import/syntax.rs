//! Swift source → a small expression tree. tree-sitter-swift gives a concrete syntax tree; the
//! importer only needs calls, members, literals, closures and `if`, so everything else becomes
//! `Other` and is reported where it matters. Conversion is depth-bounded: the source is untrusted.

use tree_sitter::{Node, Parser};

use crate::swift::unescape_identifier;

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// `name`; `raw` when it was written in backticks.
    Ident {
        name: String,
        raw: bool,
    },
    Str(Vec<Part>),
    Num(f64),
    Bool(bool),
    Member {
        base: Box<Expr>,
        name: String,
        raw: bool,
    },
    /// `.name` with an inferred base.
    Implicit(String),
    Call(Call),
    Subscript {
        base: Box<Expr>,
        args: Vec<Arg>,
    },
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
    /// `\.name` and other key paths, by text.
    KeyPath(String),
    Closure(Closure),
    Array(Vec<Expr>),
    /// Anything the importer does not read, by its tree-sitter kind.
    Other(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Text(String),
    Interpolation(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub callee: Box<Expr>,
    pub args: Vec<Arg>,
    /// Trailing closures in order; the first one has no label.
    pub closures: Vec<Closure>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arg {
    pub label: Option<String>,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Closure {
    pub label: Option<String>,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    If {
        condition: Expr,
        then: Vec<Stmt>,
        otherwise: Vec<Stmt>,
    },
    Other(String),
}

/// A declaration the importer reads: types, their properties and enum cases.
#[derive(Debug, Default)]
pub struct TypeDecl {
    pub kind: String,
    pub name: String,
    pub inherits: Vec<String>,
    pub properties: Vec<Property>,
    /// `case name = "raw"` entries of an enum.
    pub cases: Vec<(String, Option<String>)>,
}

#[derive(Debug, Default)]
pub struct Property {
    pub name: String,
    pub attributes: Vec<String>,
    pub type_name: Option<String>,
    pub initial: Option<Expr>,
    /// The statements of a computed property such as `body`.
    pub body: Option<Vec<Stmt>>,
}

pub struct Source {
    pub types: Vec<TypeDecl>,
    /// The parser recovered from syntax errors.
    pub had_errors: bool,
    /// The tree was nested deeper than the importer reads.
    pub truncated: bool,
}

pub enum ParseError {
    Grammar,
}

struct Reader<'s> {
    text: &'s [u8],
    max_depth: usize,
    max_nodes: usize,
    truncated: bool,
    nodes: usize,
}

pub fn parse(text: &str, max_depth: usize, max_nodes: usize) -> Result<Source, ParseError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_swift::LANGUAGE.into())
        .map_err(|_| ParseError::Grammar)?;
    let tree = parser.parse(text, None).ok_or(ParseError::Grammar)?;
    let root = tree.root_node();
    let mut reader = Reader {
        text: text.as_bytes(),
        max_depth,
        max_nodes,
        truncated: false,
        nodes: 0,
    };
    let mut types = vec![];
    reader.collect_types(root, 0, &mut types);
    Ok(Source {
        types,
        had_errors: root.has_error(),
        truncated: reader.truncated,
    })
}

/// Children with the field name each one has in its parent.
fn fields<'t>(node: Node<'t>) -> Vec<(Option<&'t str>, Node<'t>)> {
    let mut out = vec![];
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            out.push((cursor.field_name(), cursor.node()));
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    out
}

fn named<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

impl Reader<'_> {
    fn text(&self, node: Node<'_>) -> String {
        node.utf8_text(self.text).unwrap_or_default().to_owned()
    }

    /// Spends one node of the budget; false once the input is too large or too deep.
    fn enter(&mut self, depth: usize) -> bool {
        self.nodes += 1;
        if depth > self.max_depth || self.nodes > self.max_nodes {
            self.truncated = true;
            return false;
        }
        true
    }

    fn collect_types(&mut self, node: Node<'_>, depth: usize, out: &mut Vec<TypeDecl>) {
        if !self.enter(depth) {
            return;
        }
        for child in named(node) {
            match child.kind() {
                "class_declaration" | "protocol_declaration" => {
                    let decl = self.type_decl(child, depth + 1, out);
                    out.push(decl);
                }
                // Code inside functions and extensions is not part of a view's body.
                _ => {}
            }
        }
    }

    fn type_decl(&mut self, node: Node<'_>, depth: usize, nested: &mut Vec<TypeDecl>) -> TypeDecl {
        let mut decl = TypeDecl::default();
        for (field, child) in fields(node) {
            match (field, child.kind()) {
                (Some("declaration_kind"), _) => decl.kind = self.text(child),
                (Some("name"), _) => decl.name = self.text(child),
                (_, "inheritance_specifier") => decl.inherits.push(self.text(child)),
                (Some("body"), _) => self.type_body(child, depth + 1, &mut decl, nested),
                _ => {}
            }
        }
        decl
    }

    fn type_body(
        &mut self,
        body: Node<'_>,
        depth: usize,
        decl: &mut TypeDecl,
        nested: &mut Vec<TypeDecl>,
    ) {
        if !self.enter(depth) {
            return;
        }
        for child in named(body) {
            match child.kind() {
                "property_declaration" => {
                    if let Some(p) = self.property(child, depth + 1) {
                        decl.properties.push(p);
                    }
                }
                "enum_entry" => {
                    let name = child.child_by_field_name("name").map(|n| self.text(n));
                    let raw = child
                        .child_by_field_name("raw_value")
                        .map(|v| self.expr(v, depth + 1));
                    let raw = match raw {
                        Some(Expr::Str(parts)) => plain_string(&parts),
                        _ => None,
                    };
                    if let Some(name) = name {
                        decl.cases
                            .push((unescape_identifier(&name).to_owned(), raw));
                    }
                }
                "class_declaration" | "protocol_declaration" => {
                    let inner = self.type_decl(child, depth + 1, nested);
                    nested.push(inner);
                }
                _ => {}
            }
        }
    }

    fn property(&mut self, node: Node<'_>, depth: usize) -> Option<Property> {
        let mut p = Property::default();
        for (field, child) in fields(node) {
            match (field, child.kind()) {
                (_, "modifiers") => {
                    for m in named(child) {
                        if m.kind() == "attribute" {
                            // `@Bindable`, `@State`, `@Environment(\.x)`: the type name is enough.
                            if let Some(t) = named(m).first() {
                                p.attributes.push(self.text(*t));
                            }
                        }
                    }
                }
                (Some("name"), _) => {
                    let name = child
                        .child_by_field_name("bound_identifier")
                        .map(|n| self.text(n))
                        .unwrap_or_else(|| self.text(child));
                    p.name = unescape_identifier(&name).to_owned();
                }
                (_, "type_annotation") => {
                    p.type_name = child.child_by_field_name("name").map(|n| self.text(n));
                }
                (Some("value"), _) => p.initial = Some(self.expr(child, depth + 1)),
                (Some("computed_value"), _) => {
                    let statements = named(child).into_iter().find(|n| n.kind() == "statements");
                    p.body = Some(match statements {
                        Some(s) => self.statements(s, depth + 1),
                        None => vec![],
                    });
                }
                _ => {}
            }
        }
        (!p.name.is_empty()).then_some(p)
    }

    pub fn statements(&mut self, node: Node<'_>, depth: usize) -> Vec<Stmt> {
        if !self.enter(depth) {
            return vec![];
        }
        named(node)
            .into_iter()
            .filter(|n| n.kind() != "comment" && n.kind() != "multiline_comment")
            .map(|n| self.statement(n, depth + 1))
            .collect()
    }

    fn statement(&mut self, node: Node<'_>, depth: usize) -> Stmt {
        if node.kind() == "if_statement" {
            return self.if_statement(node, depth);
        }
        let kind = node.kind();
        if kind.ends_with("_declaration") || kind.ends_with("_statement") {
            return Stmt::Other(kind.to_owned());
        }
        Stmt::Expr(self.expr(node, depth))
    }

    fn if_statement(&mut self, node: Node<'_>, depth: usize) -> Stmt {
        if !self.enter(depth) {
            return Stmt::Other("if_statement".to_owned());
        }
        let mut conditions = vec![];
        let mut then = None;
        let mut otherwise = vec![];
        let mut after_else = false;
        for (field, child) in fields(node) {
            match (field, child.kind()) {
                (Some("condition"), _) => conditions.push(self.expr(child, depth + 1)),
                (_, "else") => after_else = true,
                (_, "statements") if !after_else && then.is_none() => {
                    then = Some(self.statements(child, depth + 1));
                }
                (_, "statements") if after_else => otherwise = self.statements(child, depth + 1),
                (_, "if_statement") if after_else => {
                    otherwise = vec![self.if_statement(child, depth + 1)]
                }
                _ => {}
            }
        }
        // `if a, b` is `if a && b`; `if let` and other bindings are not conditions Weft can say.
        let condition = conditions
            .into_iter()
            .reduce(|left, right| Expr::Binary {
                left: Box::new(left),
                op: "&&".to_owned(),
                right: Box::new(right),
            })
            .unwrap_or_else(|| Expr::Other("condition".to_owned()));
        Stmt::If {
            condition,
            then: then.unwrap_or_default(),
            otherwise,
        }
    }

    pub fn expr(&mut self, node: Node<'_>, depth: usize) -> Expr {
        if !self.enter(depth) {
            return Expr::Other("too deep".to_owned());
        }
        let d = depth + 1;
        match node.kind() {
            "simple_identifier" => {
                let text = self.text(node);
                Expr::Ident {
                    raw: text.starts_with('`'),
                    name: unescape_identifier(&text).to_owned(),
                }
            }
            "self_expression" => Expr::Ident {
                name: "self".to_owned(),
                raw: false,
            },
            "line_string_literal" => Expr::Str(self.string_parts(node, d)),
            "integer_literal" | "hex_literal" | "bin_literal" | "oct_literal" | "real_literal" => {
                match parse_number(&self.text(node)) {
                    Some(n) => Expr::Num(n),
                    None => Expr::Other(node.kind().to_owned()),
                }
            }
            "boolean_literal" => Expr::Bool(self.text(node) == "true"),
            "navigation_expression" => self.navigation(node, d),
            "prefix_expression" => self.prefix(node, d),
            "call_expression" => self.call(node, d),
            "tuple_expression" => {
                // A parenthesised expression is a one-element tuple without a label.
                let items = named(node);
                match items.as_slice() {
                    [only] => self.expr(*only, d),
                    _ => Expr::Other("tuple_expression".to_owned()),
                }
            }
            "equality_expression"
            | "comparison_expression"
            | "conjunction_expression"
            | "disjunction_expression"
            | "additive_expression"
            | "multiplicative_expression" => self.binary(node, d),
            "key_path_expression" => Expr::KeyPath(self.text(node)),
            "array_literal" => {
                let items = named(node);
                Expr::Array(items.into_iter().map(|i| self.expr(i, d)).collect())
            }
            // `x!` and `x?` read the same value for the importer's purposes.
            "postfix_expression" => match node.child_by_field_name("target") {
                Some(target) => self.expr(target, d),
                None => Expr::Other("postfix_expression".to_owned()),
            },
            "lambda_literal" => Expr::Closure(self.closure(node, None, d)),
            other => Expr::Other(other.to_owned()),
        }
    }

    fn navigation(&mut self, node: Node<'_>, depth: usize) -> Expr {
        let target = node.child_by_field_name("target");
        let suffix = node
            .child_by_field_name("suffix")
            .and_then(|s| s.child_by_field_name("suffix"));
        let Some(suffix) = suffix else {
            return Expr::Other("navigation_expression".to_owned());
        };
        let text = self.text(suffix);
        let name = unescape_identifier(&text).to_owned();
        let raw = text.starts_with('`');
        match target {
            Some(t) => Expr::Member {
                base: Box::new(self.expr(t, depth)),
                name,
                raw,
            },
            None => Expr::Implicit(name),
        }
    }

    fn prefix(&mut self, node: Node<'_>, depth: usize) -> Expr {
        let op = node
            .child_by_field_name("operation")
            .map(|o| self.text(o))
            .unwrap_or_default();
        let Some(target) = node.child_by_field_name("target") else {
            return Expr::Other("prefix_expression".to_owned());
        };
        match op.as_str() {
            "." => match self.expr(target, depth) {
                Expr::Ident { name, .. } => Expr::Implicit(name),
                _ => Expr::Other("prefix_expression".to_owned()),
            },
            "!" => Expr::Not(Box::new(self.expr(target, depth))),
            "-" => Expr::Neg(Box::new(self.expr(target, depth))),
            _ => Expr::Other("prefix_expression".to_owned()),
        }
    }

    fn binary(&mut self, node: Node<'_>, depth: usize) -> Expr {
        let parts = fields(node);
        let mut operands = vec![];
        let mut op = String::new();
        for (_, child) in parts {
            if child.is_named() && !child.kind().ends_with("_operator") {
                operands.push(child);
            } else {
                op = self.text(child);
            }
        }
        match operands.as_slice() {
            [left, right] => Expr::Binary {
                left: Box::new(self.expr(*left, depth)),
                op,
                right: Box::new(self.expr(*right, depth)),
            },
            _ => Expr::Other(node.kind().to_owned()),
        }
    }

    fn call(&mut self, node: Node<'_>, depth: usize) -> Expr {
        let mut callee = None;
        let mut suffix = None;
        for child in named(node) {
            if child.kind() == "call_suffix" {
                suffix = Some(child);
            } else if callee.is_none() {
                callee = Some(child);
            }
        }
        let (Some(callee), Some(suffix)) = (callee, suffix) else {
            return Expr::Other("call_expression".to_owned());
        };
        let callee = self.expr(callee, depth);
        let mut args = vec![];
        let mut closures = vec![];
        let mut subscript = false;
        let mut label: Option<String> = None;
        for (field, child) in fields(suffix) {
            match (field, child.kind()) {
                (_, "value_arguments") => {
                    subscript = fields(child)
                        .first()
                        .is_some_and(|(_, n)| self.text(*n) == "[");
                    for arg in named(child) {
                        if arg.kind() == "value_argument" {
                            args.push(self.argument(arg, depth));
                        }
                    }
                }
                (Some("name"), _) => {
                    let text = self.text(child);
                    label = Some(unescape_identifier(&text).to_owned());
                }
                (_, "lambda_literal") => {
                    let closure = self.closure(child, label.take(), depth);
                    closures.push(closure);
                }
                _ => {}
            }
        }
        if subscript {
            return Expr::Subscript {
                base: Box::new(callee),
                args,
            };
        }
        Expr::Call(Call {
            callee: Box::new(callee),
            args,
            closures,
        })
    }

    fn argument(&mut self, node: Node<'_>, depth: usize) -> Arg {
        let label = node
            .child_by_field_name("name")
            .map(|n| unescape_identifier(&self.text(n)).to_owned());
        let value = match node.child_by_field_name("value") {
            Some(v) => self.expr(v, depth),
            None => Expr::Other("value_argument".to_owned()),
        };
        Arg { label, value }
    }

    fn closure(&mut self, node: Node<'_>, label: Option<String>, depth: usize) -> Closure {
        let mut params = vec![];
        let mut body = vec![];
        for child in named(node) {
            match child.kind() {
                "lambda_function_type" => {
                    for list in named(child) {
                        if list.kind() == "lambda_function_type_parameters" {
                            for p in named(list) {
                                let name = p
                                    .child_by_field_name("name")
                                    .map(|n| self.text(n))
                                    .unwrap_or_else(|| self.text(p));
                                params.push(unescape_identifier(&name).to_owned());
                            }
                        }
                    }
                }
                "statements" => body = self.statements(child, depth),
                _ => {}
            }
        }
        Closure {
            label,
            params,
            body,
        }
    }

    fn string_parts(&mut self, node: Node<'_>, depth: usize) -> Vec<Part> {
        let mut parts: Vec<Part> = vec![];
        for (field, child) in fields(node) {
            let piece = match (field, child.kind()) {
                (_, "line_str_text") => Part::Text(self.text(child)),
                (_, "str_escaped_char") => Part::Text(unescape(&self.text(child))),
                (Some("interpolation"), _) => {
                    let value = child
                        .child_by_field_name("value")
                        .map(|v| self.expr(v, depth))
                        .unwrap_or_else(|| Expr::Other("interpolation".to_owned()));
                    Part::Interpolation(value)
                }
                _ => continue,
            };
            match (parts.last_mut(), piece) {
                (Some(Part::Text(prev)), Part::Text(next)) => prev.push_str(&next),
                (_, piece) => parts.push(piece),
            }
        }
        parts
    }
}

/// A literal without interpolation.
pub fn plain_string(parts: &[Part]) -> Option<String> {
    match parts {
        [] => Some(String::new()),
        [Part::Text(t)] => Some(t.clone()),
        _ => None,
    }
}

fn unescape(escape: &str) -> String {
    match escape {
        "\\n" => "\n".to_owned(),
        "\\r" => "\r".to_owned(),
        "\\t" => "\t".to_owned(),
        "\\0" => "\0".to_owned(),
        "\\\\" => "\\".to_owned(),
        "\\\"" => "\"".to_owned(),
        "\\'" => "'".to_owned(),
        other => {
            let hex = other
                .strip_prefix("\\u{")
                .and_then(|h| h.strip_suffix('}'))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .and_then(char::from_u32);
            match hex {
                Some(c) => c.to_string(),
                None => other.to_owned(),
            }
        }
    }
}

fn parse_number(text: &str) -> Option<f64> {
    let t = text.replace('_', "");
    let radix = [("0x", 16), ("0o", 8), ("0b", 2)]
        .into_iter()
        .find_map(|(p, r)| t.strip_prefix(p).map(|rest| (rest.to_owned(), r)));
    let n = match radix {
        Some((rest, r)) => i64::from_str_radix(&rest, r).ok().map(|v| v as f64),
        None => t.parse::<f64>().ok(),
    }?;
    n.is_finite().then_some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_escapes_read_like_swift() {
        assert_eq!(parse_number("1_000"), Some(1000.0));
        assert_eq!(parse_number("0x1F"), Some(31.0));
        assert_eq!(parse_number("2.5"), Some(2.5));
        assert_eq!(unescape("\\u{1F600}"), "😀");
        assert_eq!(unescape("\\\""), "\"");
    }
}
