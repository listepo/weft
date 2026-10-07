//! The expressions `generate` prints, read back off the syntax tree. Anything else stays a loss:
//! the generator's forms are closed, so a look-alike that is not one of them is not guessed at.

use std::collections::HashMap;

use i_slint_compiler::literals::unescape_string;
use i_slint_compiler::parser::{NodeOrToken, SyntaxKind, SyntaxNode};
use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::Value;

use crate::data::{Ty, binding_path};

/// CSS `rem` at the browser default: the ratio `generate` uses to turn a `rem` token into px.
const PX_PER_REM: f64 = 16.0;
/// Font sizes of heading levels 1 to 6, the px `generate` writes with weight 700.
const HEADING_FONT_PX: [u32; 6] = [32, 24, 19, 16, 13, 11];
const HEADING_WEIGHT: u32 = 700;

/// One `in-out property`, by the name Slint gave it.
pub(crate) struct Props {
    by_name: HashMap<String, (String, Ty)>,
}

impl Props {
    fn get(&self, name: &str) -> Option<(&str, Ty)> {
        self.by_name
            .get(name)
            .map(|(path, ty)| (path.as_str(), *ty))
    }
}

pub(crate) struct Perform {
    pub action: String,
    pub id: String,
}

pub(crate) struct Callback {
    pub name: String,
    pub performs: Vec<Perform>,
    /// `root.prop = ["a", "b"][self.current-index]`: the option values of a `ComboBox`.
    pub values: Option<(String, Vec<String>)>,
    pub residue: bool,
}

/// `current-index` as a literal position or as the binding the ternaries compare.
pub(crate) enum Index {
    At(usize),
    Path(String),
}

pub(crate) fn properties(window: &SyntaxNode) -> Props {
    let mut by_name = HashMap::new();
    for child in window.children() {
        if child.kind() != SyntaxKind::PropertyDeclaration {
            continue;
        }
        let Some(name) = declared_name(&child) else {
            continue;
        };
        let Some(ty) = scalar_type(&child) else {
            continue;
        };
        by_name.insert(name.clone(), (binding_path(&name), ty));
    }
    Props { by_name }
}

pub(crate) fn read_value(expr: &SyntaxNode, want: Ty, props: &Props) -> Option<Value> {
    match want {
        Ty::Bool => bool_read(expr, props),
        Ty::Str => string_read(expr, props),
        Ty::Float | Ty::Int => number_read(expr, props),
    }
}

/// `enabled` and `visible` are the binding read negated, so the `not` flag flips back.
pub(crate) fn flipped_bool(expr: &SyntaxNode, props: &Props) -> Option<Value> {
    match read_value(expr, Ty::Bool, props)? {
        Value::Bool(value) => Some(Value::Bool(!value)),
        Value::Bind { bind, not } => Some(Value::Bind { bind, not: !not }),
        other => Some(other),
    }
}

pub(crate) fn event_of(callback: &str) -> Option<&'static str> {
    match callback {
        "clicked" | "activated" => Some("press"),
        "toggled" | "edited" | "changed" | "selected" => Some("change"),
        "accepted" => Some("submit"),
        _ => None,
    }
}

pub(crate) fn callbacks(el: &SyntaxNode, props: &Props) -> Vec<Callback> {
    el.children()
        .filter(|child| child.kind() == SyntaxKind::CallbackConnection)
        .map(|child| callback(&child, props))
        .collect()
}

pub(crate) fn string_list(expr: &SyntaxNode) -> Option<Vec<String>> {
    let node = peel(expr);
    if node.kind() != SyntaxKind::Array {
        return None;
    }
    let mut out = Vec::new();
    for child in node.children() {
        out.push(lone_string(&child)?);
    }
    Some(out)
}

pub(crate) fn current_index(expr: &SyntaxNode, props: &Props) -> Option<Index> {
    if let Some(n) = plain_usize(expr) {
        return Some(Index::At(n));
    }
    let node = peel(expr);
    if node.kind() != SyntaxKind::ConditionalExpression {
        return None;
    }
    let kids: Vec<_> = node.children().collect();
    let [cond, yes, no] = kids.as_slice() else {
        return None;
    };
    let path = eq_path(cond, props)?;
    plain_usize(yes)?;
    match current_index(no, props)? {
        Index::At(0) => Some(Index::Path(path)),
        Index::Path(other) if other == path => Some(Index::Path(path)),
        Index::At(_) | Index::Path(_) => None,
    }
}

/// A px length that equals exactly one dimension token. Two tokens of the same px cannot be
/// told apart, so the spacing stays a loss.
pub(crate) fn spacing_token(expr: &SyntaxNode, tokens: &IndexMap<String, Token>) -> Option<String> {
    let text = lone_number(expr)?;
    let digits = text.strip_suffix("px")?;
    let px = digits.parse::<f64>().ok()?;
    if !px.is_finite() {
        return None;
    }
    let mut found = None;
    for (name, token) in tokens {
        if token.kind != "dimension" {
            continue;
        }
        let Some(value) = token_px(token) else {
            continue;
        };
        if value == px {
            if found.is_some() {
                return None;
            }
            found = Some(name.clone());
        }
    }
    found
}

pub(crate) fn heading_level(size: &SyntaxNode, weight: &SyntaxNode) -> Option<u32> {
    let text = lone_number(size)?;
    let digits = text.strip_suffix("px")?;
    let px = digits.parse::<u32>().ok()?;
    let weight = lone_number(weight)?;
    if weight.parse::<u32>().ok()? != HEADING_WEIGHT {
        return None;
    }
    HEADING_FONT_PX
        .iter()
        .position(|&size| size == px)
        .map(|index| index as u32 + 1)
}

fn callback(node: &SyntaxNode, props: &Props) -> Callback {
    let mut out = Callback {
        name: callback_name(node).unwrap_or_default(),
        performs: Vec::new(),
        values: None,
        residue: false,
    };
    let body = node
        .child_node(SyntaxKind::CodeBlock)
        .map(|block| block.children().collect::<Vec<_>>())
        .unwrap_or_else(|| {
            node.children()
                .filter(|child| child.kind() == SyntaxKind::Expression)
                .collect()
        });
    if body.is_empty() {
        out.residue = true;
    }
    for statement in body {
        classify(&statement, props, &mut out);
    }
    out
}

fn classify(node: &SyntaxNode, props: &Props, out: &mut Callback) {
    let node = peel(node);
    if let Some(call) = perform_of(&node) {
        out.performs.push(call);
        return;
    }
    if node.kind() == SyntaxKind::SelfAssignment
        && out.values.is_none()
        && let Some(values) = write_back(&node, props)
    {
        out.values = Some(values);
        return;
    }
    out.residue = true;
}

fn perform_of(node: &SyntaxNode) -> Option<Perform> {
    let node = peel(node);
    if node.kind() != SyntaxKind::FunctionCallExpression {
        return None;
    }
    let mut kids = node.children();
    let callee = kids.next()?;
    let (recv, method) = member(&callee)?;
    if recv != "root" || method != "perform" {
        return None;
    }
    let action = lone_string(&kids.next()?)?;
    let id = lone_string(&kids.next()?)?;
    kids.next().is_none().then_some(Perform { action, id })
}

fn write_back(node: &SyntaxNode, props: &Props) -> Option<(String, Vec<String>)> {
    if !has_token(node, SyntaxKind::Equal) {
        return None;
    }
    let mut kids = node.children();
    let left = kids.next()?;
    let right = kids.next()?;
    let (recv, name) = member(&left)?;
    if recv != "root" {
        return None;
    }
    let (path, ty) = props.get(&name)?;
    if ty != Ty::Str {
        return None;
    }
    let index = peel(&right);
    if index.kind() != SyntaxKind::IndexExpression {
        return None;
    }
    let mut parts = index.children();
    let array = parts.next()?;
    let at = parts.next()?;
    let (host, field) = member(&at)?;
    if host != "self" || field != "current-index" {
        return None;
    }
    Some((path.to_owned(), string_list(&array)?))
}

fn callback_name(node: &SyntaxNode) -> Option<String> {
    let mut names = Vec::new();
    for child in node.children_with_tokens() {
        let Some(token) = child.into_token() else {
            continue;
        };
        if token.kind() == SyntaxKind::FatArrow {
            break;
        }
        if token.kind() == SyntaxKind::Identifier {
            names.push(token.text().to_string());
        }
    }
    match names.as_slice() {
        [name] => Some(name.clone()),
        _ => None,
    }
}

fn bool_read(expr: &SyntaxNode, props: &Props) -> Option<Value> {
    if let Some(value) = bool_lit(expr) {
        return Some(Value::Bool(value));
    }
    if let Some(inner) = bang(expr) {
        let (path, ty) = root_prop(&inner, props)?;
        return (ty == Ty::Bool).then_some(Value::Bind {
            bind: path,
            not: true,
        });
    }
    if let Some((left, op, right)) = binary(expr) {
        let (path, ty) = root_prop(&left, props)?;
        let not = match op {
            SyntaxKind::NotEqual => false,
            SyntaxKind::EqualEqual => true,
            _ => return None,
        };
        let matches_type = match ty {
            Ty::Str => lone_string(&right).as_deref() == Some(""),
            Ty::Float | Ty::Int => is_zero(&right),
            Ty::Bool => false,
        };
        return matches_type.then_some(Value::Bind { bind: path, not });
    }
    let (path, ty) = root_prop(expr, props)?;
    (ty == Ty::Bool).then_some(Value::Bind {
        bind: path,
        not: false,
    })
}

fn string_read(expr: &SyntaxNode, props: &Props) -> Option<Value> {
    if let Some(text) = lone_string(expr) {
        return Some(Value::String(text));
    }
    let (path, _) = root_prop(expr, props)?;
    Some(Value::Bind {
        bind: path,
        not: false,
    })
}

fn number_read(expr: &SyntaxNode, props: &Props) -> Option<Value> {
    if let Some(text) = lone_number(expr) {
        if text.chars().any(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let n = text.parse::<f64>().ok()?;
        return n.is_finite().then_some(Value::Number(n));
    }
    let (path, ty) = root_prop(expr, props)?;
    matches!(ty, Ty::Float | Ty::Int).then_some(Value::Bind {
        bind: path,
        not: false,
    })
}

fn eq_path(expr: &SyntaxNode, props: &Props) -> Option<String> {
    let (left, op, right) = binary(expr)?;
    if op != SyntaxKind::EqualEqual {
        return None;
    }
    let (path, ty) = root_prop(&left, props)?;
    if ty != Ty::Str || lone_string(&right).is_none() {
        return None;
    }
    Some(path)
}

fn root_prop(expr: &SyntaxNode, props: &Props) -> Option<(String, Ty)> {
    let (recv, name) = member(expr)?;
    if recv != "root" {
        return None;
    }
    let (path, ty) = props.get(&name)?;
    Some((path.to_owned(), ty))
}

fn binary(expr: &SyntaxNode) -> Option<(SyntaxNode, SyntaxKind, SyntaxNode)> {
    let node = peel(expr);
    if node.kind() != SyntaxKind::BinaryExpression {
        return None;
    }
    let op = node.children_with_tokens().find_map(|child| {
        let token = child.into_token()?;
        matches!(token.kind(), SyntaxKind::EqualEqual | SyntaxKind::NotEqual)
            .then_some(token.kind())
    })?;
    let mut kids = node.children();
    Some((kids.next()?, op, kids.next()?))
}

fn bang(expr: &SyntaxNode) -> Option<SyntaxNode> {
    let node = peel(expr);
    if node.kind() != SyntaxKind::UnaryOpExpression || !has_token(&node, SyntaxKind::Bang) {
        return None;
    }
    node.children().next()
}

fn member(expr: &SyntaxNode) -> Option<(String, String)> {
    // `root.name` is a qualified name: the parser eats the dot before member-access does.
    let node = peel(expr);
    if !matches!(
        node.kind(),
        SyntaxKind::QualifiedName | SyntaxKind::MemberAccess
    ) {
        return None;
    }
    let mut names = Vec::new();
    collect_idents(&node, &mut names);
    match names.as_slice() {
        [recv, name] => Some((recv.clone(), name.clone())),
        _ => None,
    }
}

fn bool_lit(expr: &SyntaxNode) -> Option<bool> {
    let node = peel(expr);
    if matches!(
        node.kind(),
        SyntaxKind::BinaryExpression
            | SyntaxKind::UnaryOpExpression
            | SyntaxKind::MemberAccess
            | SyntaxKind::ConditionalExpression
            | SyntaxKind::FunctionCallExpression
    ) {
        return None;
    }
    let mut names = Vec::new();
    collect_idents(&node, &mut names);
    match names.as_slice() {
        [name] if name == "true" => Some(true),
        [name] if name == "false" => Some(false),
        _ => None,
    }
}

fn is_zero(expr: &SyntaxNode) -> bool {
    lone_number(expr).is_some_and(|text| {
        !text.chars().any(|c| c.is_ascii_alphabetic()) && text.parse::<f64>().ok() == Some(0.0)
    })
}

fn plain_usize(expr: &SyntaxNode) -> Option<usize> {
    let text = lone_number(expr)?;
    if text.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    text.parse().ok()
}

fn lone_number(expr: &SyntaxNode) -> Option<String> {
    lone_token(expr, SyntaxKind::NumberLiteral)
}

fn lone_string(expr: &SyntaxNode) -> Option<String> {
    let text = lone_token(expr, SyntaxKind::StringLiteral)?;
    unescape_string(&text).map(|value| value.to_string())
}

fn lone_token(expr: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    let node = peel(expr);
    if node.kind() != SyntaxKind::Expression || node.children().next().is_some() {
        return None;
    }
    let mut found = None;
    for child in node.children_with_tokens() {
        let token = child.into_token()?;
        if matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comment) {
            continue;
        }
        if token.kind() == kind && found.is_none() {
            found = Some(token.text().to_string());
        } else {
            return None;
        }
    }
    found
}

fn peel(node: &SyntaxNode) -> SyntaxNode {
    let mut node = node.clone();
    loop {
        if matches!(
            node.kind(),
            SyntaxKind::BindingExpression | SyntaxKind::Expression
        ) {
            let mut kids = node.children();
            let Some(only) = kids.next() else { break };
            if kids.next().is_some() {
                break;
            }
            node = only;
            continue;
        }
        break;
    }
    node
}

fn collect_idents(node: &SyntaxNode, names: &mut Vec<String>) {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Identifier => {
                names.push(token.text().to_string());
            }
            NodeOrToken::Node(inner) => collect_idents(&inner, names),
            NodeOrToken::Token(_) => {}
        }
    }
}

fn has_token(node: &SyntaxNode, kind: SyntaxKind) -> bool {
    node.children_with_tokens()
        .any(|child| child.into_token().is_some_and(|token| token.kind() == kind))
}

fn declared_name(node: &SyntaxNode) -> Option<String> {
    let id = node.child_node(SyntaxKind::DeclaredIdentifier)?;
    let mut names = Vec::new();
    collect_idents(&id, &mut names);
    match names.as_slice() {
        [name] => Some(name.clone()),
        _ => None,
    }
}

fn scalar_type(decl: &SyntaxNode) -> Option<Ty> {
    let ty = decl.child_node(SyntaxKind::Type)?;
    match ty.text().to_string().trim() {
        "string" => Some(Ty::Str),
        "bool" => Some(Ty::Bool),
        "float" => Some(Ty::Float),
        "int" => Some(Ty::Int),
        _ => None,
    }
}

fn token_px(token: &Token) -> Option<f64> {
    let n = token.value.get("value")?.as_f64()?;
    match token.value.get("unit")?.as_str()? {
        "px" => Some(n),
        "rem" => Some(n * PX_PER_REM),
        _ => None,
    }
}
