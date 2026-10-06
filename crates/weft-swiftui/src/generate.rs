//! Weft document → one SwiftUI source file. Each kind has one generated form (README.md, "Mapping"),
//! chosen so that `import_swiftui` can read it back: a prop that SwiftUI cannot express, or whose
//! native form would read the same as its absence, travels in a `.weftProp(name, value)` marker.
//!
//! The document is untrusted. Its strings reach the output only as escaped Swift string literals,
//! and identifiers come from this module or from names that are checked against Swift's grammar.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_catalog::{Appearance, Token, token_types};
use weft_core::controls::{self, DateType};
use weft_core::{
    Catalog, Child, ComponentDef, Content, Diagnostic, Document, Mode, Node, PropType,
    ValidateOptions, Value, has_errors, validate_document,
};

use crate::Unsupported;
use crate::data::{Leaf, Shapes, Ty, prop_leaf};
use crate::kinds::{EACH, is_core_kind};
use crate::sample;
use crate::swift::{self, string_literal};
use crate::theme::{TOKENS_TYPE, check_token, token_expr, tokens_struct};

pub struct GenerateOptions<'a> {
    pub catalog: &'a Catalog,
    /// Resolved design tokens (`weft_catalog::load_tokens`); referenced tokens become the theme.
    pub tokens: &'a IndexMap<String, Token>,
    /// The prefix of the generated type names (`LoginModel`, `LoginScreen`…); the screen id when
    /// absent. Screens that share an id need distinct names to live in one module.
    pub name: Option<&'a str>,
    /// The view reads its tokens from the project's shared `WeftTokens` (`generate_tokens`)
    /// instead of a theme struct of its own, which keeps the file self-contained.
    pub shared_tokens: bool,
    /// Sample data: the model gets a memberwise initializer and a `sample` built from this data,
    /// which `#Preview` shows. Without it the file is as before.
    pub data: Option<&'a Json>,
    /// The light and dark tokens of the project's resolver (`weft_catalog::appearance`): a colour
    /// that differs follows the system appearance in the screen's own theme. The shared tokens
    /// take it in `generate_tokens` instead.
    pub appearance: Option<Appearance<'a>>,
}

#[derive(Debug)]
pub enum GenerateError {
    /// The document does not validate in strict mode against the catalog and tokens.
    Invalid(Vec<Diagnostic>),
    /// The document is valid, but says something the generator cannot express in Swift.
    Unsupported(Vec<Unsupported>),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenerateError::Invalid(d) => write!(f, "the document has {} diagnostics", d.len()),
            GenerateError::Unsupported(p) => {
                let list: Vec<String> = p.iter().map(ToString::to_string).collect();
                write!(f, "cannot generate SwiftUI: {}", list.join("; "))
            }
        }
    }
}

impl std::error::Error for GenerateError {}

/// The Swift source of `document`: data model, actions, theme and view, plus the helpers the
/// view uses, in one file that compiles on its own for iOS 17 and macOS 14.
pub fn generate(
    document: &Document,
    options: &GenerateOptions<'_>,
) -> Result<String, GenerateError> {
    let types = token_types(options.tokens);
    let diagnostics = validate_document(
        document,
        &ValidateOptions {
            catalog: Some(options.catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    if has_errors(&diagnostics) {
        return Err(GenerateError::Invalid(diagnostics));
    }
    let mut problems = vec![];
    check_extensions(&document.root, "", &mut problems);
    if !problems.is_empty() {
        return Err(GenerateError::Unsupported(problems));
    }
    let shapes = Shapes::infer(&document.root, options.catalog, &mut problems);
    let name = options
        .name
        .or(document.root.id.as_deref())
        .unwrap_or("screen");
    let mut g = Gen::new(
        options.catalog,
        shapes,
        &document.root,
        swift::type_prefix(name),
    );
    g.problems = problems;
    g.shared_tokens = options.shared_tokens;
    g.data = options.data;
    g.appearance = options.appearance;
    let text = g.file(&document.root, options.tokens);
    if g.problems.is_empty() {
        Ok(text)
    } else {
        Err(GenerateError::Unsupported(g.problems))
    }
}

fn element_path(parent: &str, node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{parent}/{}#{id}", node.kind),
        None => format!("{parent}/{}", node.kind),
    }
}

/// Extension elements and attributes (SPEC §8) have no SwiftUI meaning to generate.
fn check_extensions(node: &Node, path: &str, problems: &mut Vec<Unsupported>) {
    let here = element_path(path, node);
    if node.kind.starts_with("x-") {
        problems.push(Unsupported::new(
            &here,
            "extension elements are not generated",
        ));
    }
    for name in node.props.keys().filter(|k| k.starts_with("x-")) {
        problems.push(Unsupported::new(
            format!("{here}/@{name}"),
            "extension attributes are not generated",
        ));
    }
    let lists = std::iter::once(&node.children).chain(node.slots.values());
    for child in lists.flatten() {
        if let Child::Node(n) = child {
            check_extensions(n, &here, problems);
        }
    }
}

/// A view: the lines of its initializer and one entry per modifier, each one or more lines.
#[derive(Default)]
struct V {
    head: Vec<String>,
    mods: Vec<Vec<String>>,
    /// Groups its children for accessibility (`.accessibilityElement(children: .contain)`), so
    /// that its own identifier and label are not copied onto every child.
    container: bool,
    /// The label is the control's visible title, not an accessibility label.
    captioned: bool,
    /// A dialog's `.sheet(…)` opening line: the view is presented, not placed.
    sheet: Option<String>,
}

impl V {
    fn new(head: Vec<String>) -> Self {
        V {
            head,
            ..V::default()
        }
    }

    fn line(head: impl Into<String>) -> Self {
        V::new(vec![head.into()])
    }

    fn modifier(&mut self, line: impl Into<String>) {
        self.mods.push(vec![line.into()]);
    }

    fn render(self) -> Vec<String> {
        let mut out = self.head;
        let closed = out.last().is_some_and(|l| l.trim_end().ends_with('}'));
        let pad = if closed { "" } else { "    " };
        for m in self.mods {
            out.extend(m.into_iter().map(|l| format!("{pad}{l}")));
        }
        out
    }
}

fn indent(lines: Vec<String>) -> impl Iterator<Item = String> {
    lines
        .into_iter()
        .map(|l| if l.is_empty() { l } else { format!("    {l}") })
}

/// A stack's or grid's `material` token as the token's own view modifier (`theme.<path>` is a
/// `Surface`, see `theme`).
fn material(v: &mut V, props: &mut IndexMap<String, Value>) {
    if let Some(Value::Token(t)) = props.get("material") {
        v.modifier(format!(".modifier({})", token_expr(t)));
        props.shift_remove("material");
    }
}

/// `head {` body `}` with the body indented.
fn block(head: &str, body: Vec<String>) -> Vec<String> {
    let mut out = vec![format!("{head} {{")];
    out.extend(indent(body));
    out.push("}".to_owned());
    out
}

/// A trailing closure after another one: `} label: {` body `}`.
fn and_block(mut lines: Vec<String>, label: &str, body: Vec<String>) -> Vec<String> {
    if let Some(last) = lines.last_mut() {
        last.push_str(&format!(" {label}: {{"));
    }
    lines.extend(indent(body));
    lines.push("}".to_owned());
    lines
}

/// A number a control reads: what the document wrote, or code that reads it at run time.
enum Num {
    Absent,
    Lit(f64),
    Expr(String),
}

impl Num {
    fn lit(&self) -> Option<f64> {
        match self {
            Num::Lit(n) => Some(*n),
            _ => None,
        }
    }

    /// Swift code for the number, `default` when the document did not write one.
    fn code(&self, default: &str) -> String {
        match self {
            Num::Absent => default.to_owned(),
            Num::Lit(n) => swift::number_literal(*n),
            Num::Expr(e) => e.clone(),
        }
    }
}

/// A control's `step`: a step that is not above 0 is the default, so it goes through the helper.
/// A slider always needs one, because SwiftUI's slider without a step is continuous.
fn step_code(step: &Num, always: bool) -> String {
    match step {
        Num::Lit(n) if *n > 0.0 => swift::number_literal(*n),
        Num::Absent if !always => "1".to_owned(),
        other => format!("weftStep({})", other.code("nil")),
    }
}

/// A loop variable of an enclosing `<each>`.
struct Loop {
    name: String,
    ident: String,
    item: usize,
    /// The binding of the current item, e.g. `$model.todos[todoIndex]`.
    write: String,
    /// The absolute data path of the current item inside a Swift string literal, e.g.
    /// `$.todos.\(todoIndex)`.
    item_path: String,
}

#[derive(Clone, Copy)]
struct Ctx<'c> {
    form: Option<&'c str>,
}

/// Names the generated view declares, which a loop variable must not shadow.
const VIEW_MEMBERS: &[&str] = &[
    "body", "model", "theme", "perform", "send", "submit", "openLink", "openURL",
];

struct Gen<'a> {
    catalog: &'a Catalog,
    shapes: Shapes,
    prefix: String,
    actions: IndexMap<String, String>,
    tokens: Vec<String>,
    forms: IndexMap<String, String>,
    loop_names: HashSet<String>,
    submits: bool,
    links: bool,
    models: bool,
    shared_tokens: bool,
    /// The views the app writes for the kinds of its own catalog, by view name.
    custom: IndexMap<String, String>,
    problems: Vec<Unsupported>,
    data: Option<&'a Json>,
    appearance: Option<Appearance<'a>>,
}

impl<'a> Gen<'a> {
    fn new(catalog: &'a Catalog, shapes: Shapes, root: &Node, prefix: String) -> Self {
        let mut g = Gen {
            catalog,
            shapes,
            prefix,
            actions: IndexMap::new(),
            tokens: vec![],
            forms: IndexMap::new(),
            loop_names: HashSet::new(),
            submits: false,
            links: false,
            models: false,
            shared_tokens: false,
            custom: IndexMap::new(),
            problems: vec![],
            data: None,
            appearance: None,
        };
        g.survey(root);
        g
    }

    /// Actions, tokens, forms and loop names, in document order, before anything is printed.
    fn survey(&mut self, node: &Node) {
        for action in node.on.values() {
            if !self.actions.contains_key(action) {
                let base = swift::action_case(action);
                let mut case = base.clone();
                let mut n = 2;
                while self.actions.values().any(|c| *c == case) {
                    case = format!("{base}{n}");
                    n += 1;
                }
                self.actions.insert(action.clone(), case);
            }
        }
        for value in node.props.values() {
            if let Value::Token(path) = value
                && !self.tokens.contains(path)
            {
                self.tokens.push(path.clone());
            }
        }
        if node.kind == "form"
            && let (Some(id), Some(action)) = (&node.id, node.on.get("submit"))
        {
            self.forms.insert(id.clone(), action.clone());
        }
        if node.kind == "button" && node.props.get("submit") == Some(&Value::Bool(true)) {
            self.submits = true;
        }
        if node.kind == "link" && node.props.contains_key("href") {
            self.links = true;
        }
        if node.kind == "model" {
            self.models = true;
        }
        if node.kind == EACH
            && let Some(Value::String(name)) = node.props.get("as")
        {
            self.loop_names.insert(name.clone());
        }
        let lists = std::iter::once(&node.children).chain(node.slots.values());
        for child in lists.flatten() {
            if let Child::Node(n) = child {
                self.survey(n);
            }
        }
    }

    // ---- File ----

    fn file(&mut self, root: &Node, tokens: &IndexMap<String, Token>) -> String {
        let id = root.id.clone().unwrap_or_default();
        let p = self.prefix.clone();
        // The body is printed first: it decides which custom views the header names.
        let view = self.view(root);
        let mut out: Vec<String> = vec![
            format!(
                "// Generated by `weft swiftui` from the Weft screen {}.",
                string_literal(&id)
            ),
            "// Regenerate it from the .weft source, or edit it and read it back with".to_owned(),
            "// `weft import-swiftui`; the `weft…` helpers at the end carry what SwiftUI cannot say.".to_owned(),
        ];
        if !self.custom.is_empty() {
            let views: Vec<String> = self
                .custom
                .iter()
                .map(|(view, kind)| format!("`{view}` (`{kind}`)"))
                .collect();
            out.push(format!(
                "// The app writes the views for its own kinds: {}.",
                views.join(", ")
            ));
        }
        out.push(String::new());
        if self.models {
            out.push("import ImageIO".to_owned());
        }
        out.push("import Observation".to_owned());
        if self.models {
            out.push("import RealityKit".to_owned());
        }
        out.extend(["import SwiftUI".to_owned(), String::new()]);
        out.extend(self.model_class());
        out.push(String::new());
        out.extend(self.action_enum());
        out.push(String::new());
        out.push("/// What the host receives when the screen fires an action.".to_owned());
        out.push(format!("struct {p}Event: Sendable, Equatable {{"));
        out.push(format!("    var action: {p}Action"));
        out.push("    /// The id of the element that fired it.".to_owned());
        out.push("    var id: String".to_owned());
        out.push(
            "    /// The data path of the list item it came from, e.g. `$.todos.2`.".to_owned(),
        );
        out.push("    var item: String?".to_owned());
        out.push("}".to_owned());
        out.push(String::new());
        let used = self.tokens.clone();
        if self.shared_tokens {
            self.problems
                .extend(used.iter().filter_map(|path| check_token(path, tokens)));
        } else {
            out.push("/// The design tokens the screen uses.".to_owned());
            out.extend(tokens_struct(
                &format!("{p}Theme"),
                used.iter().map(String::as_str),
                tokens,
                self.appearance,
                &mut self.problems,
            ));
            out.push(String::new());
        }
        out.extend(view);
        out.push(String::new());
        out.push("#Preview {".to_owned());
        if self.data.is_some() {
            out.push(format!("    {p}Screen(model: .sample)"));
        } else {
            out.push(format!("    {p}Screen(model: {p}Model())"));
        }
        out.push("}".to_owned());
        out.push(String::new());
        out.extend(HELPERS.lines().map(str::to_owned));
        if self.models {
            out.extend(MODEL_HELPERS.lines().map(str::to_owned));
        }
        let mut text = out.join("\n");
        text.push('\n');
        text
    }

    fn model_class(&self) -> Vec<String> {
        let p = &self.prefix;
        let mut out = vec![
            "/// The data the screen reads and writes.".to_owned(),
            "@Observable".to_owned(),
            format!("final class {p}Model {{"),
        ];
        let mut names = HashMap::new();
        out.extend(indent(self.fields(0, "", &mut names)));
        out.push("}".to_owned());
        if let Some(data) = self.data {
            out.push(String::new());
            out.extend(sample::extension(
                &format!("{p}Model"),
                &self.shapes,
                &names,
                data,
            ));
        }
        out
    }

    /// The stored properties of the struct at `at`, then the nested types they use. `scope` is the
    /// struct's name inside the model class ("" for the class) and `names` collects every nested
    /// struct's, for the sample.
    fn fields(&self, at: usize, scope: &str, names: &mut HashMap<usize, String>) -> Vec<String> {
        let mut props = vec![];
        let mut nested: Vec<Vec<String>> = vec![];
        let mut taken: Vec<String> = vec![];
        let mut members = vec![];
        let shape = &self.shapes.nodes[at];
        for (name, &child) in &shape.fields {
            let ty = self.shapes.type_at(child).clone();
            let (swift_ty, default) =
                self.declare(&ty, name, scope, &mut taken, &mut nested, names);
            let member = swift::member(name);
            props.push(format!("var {member}: {swift_ty} = {default}"));
            members.push(format!("{member}: {swift_ty} = {default}"));
        }
        // A class has no memberwise initializer of its own; the structs get Swift's.
        if at == 0 && self.data.is_some() && !members.is_empty() {
            props.push(String::new());
            props.push(
                "/// Every property, defaulting to the value it is declared with.".to_owned(),
            );
            props.push("init(".to_owned());
            let last = members.len() - 1;
            for (i, m) in members.iter().enumerate() {
                props.push(format!("    {m}{}", if i < last { "," } else { "" }));
            }
            props.push(") {".to_owned());
            for name in shape.fields.keys() {
                let member = swift::member(name);
                props.push(format!("    self.{member} = {member}"));
            }
            props.push("}".to_owned());
        }
        for n in nested {
            props.push(String::new());
            props.extend(n);
        }
        props
    }

    /// The Swift type and default value of a field, declaring the struct types it needs.
    fn declare(
        &self,
        ty: &Ty,
        name: &str,
        scope: &str,
        taken: &mut Vec<String>,
        nested: &mut Vec<Vec<String>>,
        names: &mut HashMap<usize, String>,
    ) -> (String, String) {
        match ty {
            Ty::String => ("String".to_owned(), "\"\"".to_owned()),
            Ty::Bool => ("Bool".to_owned(), "false".to_owned()),
            Ty::Int => ("Int".to_owned(), "0".to_owned()),
            Ty::Double => ("Double".to_owned(), "0".to_owned()),
            Ty::Struct(at) => {
                let type_name = unique_type(&format!("{}Data", upper_first(name)), taken);
                let qualified = if scope.is_empty() {
                    type_name.clone()
                } else {
                    format!("{scope}.{type_name}")
                };
                names.insert(*at, qualified.clone());
                let mut lines = vec![format!("struct {type_name}: Hashable, Sendable {{")];
                lines.extend(indent(self.fields(*at, &qualified, names)));
                lines.push("}".to_owned());
                nested.push(lines);
                (type_name.clone(), format!("{type_name}()"))
            }
            Ty::Array(item) => {
                let item_name = format!("{}Item", upper_first(name));
                let (inner, _) = self.declare(item, &item_name, scope, taken, nested, names);
                (format!("[{inner}]"), "[]".to_owned())
            }
        }
    }

    fn action_enum(&self) -> Vec<String> {
        let p = &self.prefix;
        let mut out = vec!["/// The host actions the screen names.".to_owned()];
        if self.actions.is_empty() {
            out.push(format!(
                "enum {p}Action: Equatable, CaseIterable, Sendable {{}}"
            ));
            return out;
        }
        out.push(format!("enum {p}Action: String, CaseIterable, Sendable {{"));
        for (name, case) in &self.actions {
            out.push(format!("    case {case} = {}", string_literal(name)));
        }
        out.push("}".to_owned());
        out
    }

    fn view(&mut self, root: &Node) -> Vec<String> {
        let p = self.prefix.clone();
        let mut out = vec![
            format!("struct {p}Screen: View {{"),
            format!("    @Bindable var model: {p}Model"),
            if self.shared_tokens {
                format!("    var theme = {TOKENS_TYPE}()")
            } else {
                format!("    var theme = {p}Theme()")
            },
            format!("    var perform: ({p}Event) -> Void = {{ _ in }}"),
        ];
        if self.links {
            out.push("    @Environment(\\.openURL) private var openURL".to_owned());
        }
        out.push(String::new());
        let body = self.node(root, &mut vec![], Ctx { form: None }, "");
        out.extend(indent(block("var body: some View", body)));
        out.push(String::new());
        out.push(format!(
            "    private func send(_ action: {p}Action, _ id: String, item: String? = nil) {{"
        ));
        out.push(format!(
            "        perform({p}Event(action: action, id: id, item: item))"
        ));
        out.push("    }".to_owned());
        if self.submits {
            out.push(String::new());
            out.push("    /// A submit button fires its form's submit action.".to_owned());
            out.push("    private func submit(_ form: String) {".to_owned());
            out.push("        switch form {".to_owned());
            for (form, action) in &self.forms {
                let case = self.case(action);
                out.push(format!(
                    "        case {}: send(.{case}, {})",
                    string_literal(form),
                    string_literal(form)
                ));
            }
            out.push("        default: break".to_owned());
            out.push("        }".to_owned());
            out.push("    }".to_owned());
        }
        if self.links {
            out.push(String::new());
            out.push("    private func openLink(_ href: String) {".to_owned());
            out.push("        if let url = weftURL(href) { openURL(url) }".to_owned());
            out.push("    }".to_owned());
        }
        out.push("}".to_owned());
        out
    }

    fn case(&self, action: &str) -> String {
        self.actions.get(action).cloned().unwrap_or_default()
    }

    // ---- Values ----

    /// The read expression, Swift type and binding base of a bound path.
    fn resolve(&self, bind: &str, loops: &[Loop]) -> Option<(String, String, Ty)> {
        let rest = bind.strip_prefix('$')?;
        let mut segments = rest.split('.');
        let head = segments.next()?;
        let (mut read, mut write) = if head.is_empty() {
            ("model".to_owned(), "$model".to_owned())
        } else {
            let l = loops.iter().rev().find(|l| l.name == head)?;
            (l.ident.clone(), l.write.clone())
        };
        for segment in segments {
            let m = swift::member(segment);
            read.push('.');
            read.push_str(&m);
            write.push('.');
            write.push_str(&m);
        }
        let scope: Vec<(String, usize)> = loops.iter().map(|l| (l.name.clone(), l.item)).collect();
        let at = self.shapes.find(bind, &scope)?;
        Some((read, write, self.shapes.type_at(at).clone()))
    }

    /// A Swift expression that reads `value` as text, a flag or a whole number.
    fn expr(&self, value: &Value, leaf: Leaf, loops: &[Loop]) -> String {
        match value {
            Value::String(s) => match leaf {
                Leaf::Text => string_literal(s),
                Leaf::Bool => (!s.is_empty() && s != "false").to_string(),
                Leaf::Int => swift::number_literal(s.trim().parse::<f64>().unwrap_or(0.0)),
                Leaf::Double => {
                    swift::number_literal(controls::numeric(None, Some(s)).unwrap_or(0.0))
                }
            },
            Value::Number(n) => match leaf {
                Leaf::Text => string_literal(&swift::number_literal(*n)),
                Leaf::Bool => (*n != 0.0).to_string(),
                Leaf::Int | Leaf::Double => swift::number_literal(*n),
            },
            Value::Bool(b) => match leaf {
                Leaf::Text => string_literal(&b.to_string()),
                Leaf::Bool => b.to_string(),
                Leaf::Int | Leaf::Double => if *b { "1" } else { "0" }.to_owned(),
            },
            Value::Token(path) => token_expr(path),
            Value::Bind { bind, not } => {
                let Some((read, _, ty)) = self.resolve(bind, loops) else {
                    return string_literal("");
                };
                match (leaf, *not) {
                    (Leaf::Bool, false) => match ty {
                        Ty::Bool => read,
                        Ty::String => format!("!{read}.isEmpty"),
                        Ty::Int | Ty::Double => format!("{read} != 0"),
                        _ => format!("weftOn({read})"),
                    },
                    (Leaf::Bool, true) => match ty {
                        Ty::Bool => format!("!{read}"),
                        Ty::String => format!("{read}.isEmpty"),
                        Ty::Int | Ty::Double => format!("{read} == 0"),
                        _ => format!("!weftOn({read})"),
                    },
                    (Leaf::Text, _) => match ty {
                        Ty::String => read,
                        _ => format!("weftText({read})"),
                    },
                    (Leaf::Int, _) => match ty {
                        Ty::Int => read,
                        _ => format!("weftInt({read})"),
                    },
                    (Leaf::Double, _) => match ty {
                        Ty::Double => read,
                        Ty::Int => format!("Double({read})"),
                        _ => format!("weftDouble({read})"),
                    },
                }
            }
        }
    }

    /// The `Binding` a control writes through, and whether the value needs a marker because a
    /// literal reads the same as the prop's absence.
    fn binding(&self, value: Option<&Value>, leaf: Leaf, loops: &[Loop]) -> (String, bool) {
        match value {
            Some(Value::Bind { bind, not: false }) => match self.resolve(bind, loops) {
                Some((_, write, _)) => (write, false),
                None => (".constant(\"\")".to_owned(), false),
            },
            Some(v) => {
                let lit = self.expr(v, leaf, loops);
                let blank = matches!(lit.as_str(), "\"\"" | "false");
                (format!(".constant({lit})"), blank)
            }
            None => match leaf {
                Leaf::Bool => (".constant(false)".to_owned(), false),
                Leaf::Int | Leaf::Double => (".constant(0)".to_owned(), false),
                Leaf::Text => (".constant(\"\")".to_owned(), false),
            },
        }
    }

    /// `send(.case, "id")`, with the item path inside a loop.
    fn send(&self, action: &str, id: &str, loops: &[Loop]) -> String {
        let case = self.case(action);
        match loops.last() {
            Some(l) => format!(
                "send(.{case}, {}, item: \"{}\")",
                string_literal(id),
                l.item_path
            ),
            None => format!("send(.{case}, {})", string_literal(id)),
        }
    }

    /// `Text(…)` for a text-content kind: content as a literal, the `text` prop as `verbatim:`
    /// or an expression.
    fn text_view(
        &self,
        node: &Node,
        props: &mut IndexMap<String, Value>,
        loops: &[Loop],
    ) -> String {
        if let Some(v) = props.shift_remove("text") {
            return match v {
                Value::String(s) => format!("Text(verbatim: {})", string_literal(&s)),
                other => format!("Text({})", self.expr(&other, Leaf::Text, loops)),
            };
        }
        let text: String = node
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("Text({})", string_literal(&text))
    }

    // ---- Elements ----

    fn children(
        &mut self,
        list: &[Child],
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> Vec<String> {
        let mut out = vec![];
        for child in list {
            match child {
                Child::Node(n) => out.extend(self.node(n, loops, ctx, path)),
                Child::Text(t) => out.push(format!("Text({})", string_literal(t))),
            }
        }
        out
    }

    fn slot(
        &mut self,
        node: &Node,
        name: &str,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> Option<Vec<String>> {
        let list = node.slots.get(name)?;
        let here = format!("{path}/slot[{name}]");
        Some(self.children(list, loops, ctx, &here))
    }

    fn node(
        &mut self,
        node: &Node,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        parent: &str,
    ) -> Vec<String> {
        let path = element_path(parent, node);
        if node.kind == EACH {
            return self.each(node, loops, ctx, &path);
        }
        let id = node.id.clone().unwrap_or_default();
        let mut props = node.props.clone();
        let mut on = node.on.clone();
        let label = props.shift_remove("label");
        let hidden = props.shift_remove("hidden");
        // A dialog is drawn into its own layer, so its tilt stays a marker (SPEC §2.2).
        let tilt = if node.kind == "dialog" {
            vec![]
        } else {
            tilt_modifiers(&mut props)
        };
        let mut v = self.kind(
            node,
            &id,
            &mut props,
            &mut on,
            label.as_ref(),
            loops,
            ctx,
            &path,
        );

        // Markers: what SwiftUI cannot say, sorted by name like canonical attributes.
        let mut leftovers: Vec<(String, Value)> = props.into_iter().collect();
        if label.is_some()
            && !v.captioned
            && matches!(node.kind.as_str(), "stack" | "grid" | "text")
        {
            // SPEC §2.2: role-none kinds have no accessible name, so `label` has no effect.
            if let Some(l) = &label {
                leftovers.push(("label".to_owned(), l.clone()));
            }
        }
        leftovers.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, value) in leftovers {
            let leaf = prop_leaf(self.catalog, &node.kind, &name).map(|(l, _)| l);
            let Some(leaf) = leaf else {
                self.problems.push(Unsupported::new(
                    format!("{path}/@{name}"),
                    "this attribute has no SwiftUI form",
                ));
                continue;
            };
            let expr = self.expr(&value, leaf, loops);
            v.modifier(format!(".weftProp({}, {expr})", string_literal(&name)));
        }
        for modifier in tilt {
            v.modifier(modifier);
        }
        for event in on.keys() {
            self.problems.push(Unsupported::new(
                format!("{path}/@on-{event}"),
                "this event has no SwiftUI form",
            ));
        }
        if v.container {
            v.modifier(".accessibilityElement(children: .contain)");
        }
        if node.kind == "dialog" {
            v.modifier(".accessibilityAddTraits(.isModal)");
        }
        if let Some(l) = &label
            && !v.captioned
            && !matches!(node.kind.as_str(), "stack" | "grid" | "text")
        {
            v.modifier(format!(
                ".accessibilityLabel({})",
                self.expr(l, Leaf::Text, loops)
            ));
        }
        v.modifier(format!(".accessibilityIdentifier({})", string_literal(&id)));
        if node.kind == "dialog" {
            return self.dialog_anchor(v, hidden.as_ref(), loops);
        }
        if let Some(h) = &hidden {
            v.modifier(format!(".weftHidden({})", self.expr(h, Leaf::Bool, loops)));
        }
        v.render()
    }

    fn each(
        &mut self,
        node: &Node,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> Vec<String> {
        let (Some(Value::Bind { bind, .. }), Some(Value::String(name))) =
            (node.props.get("in"), node.props.get("as"))
        else {
            return vec![];
        };
        let Some((read, write, _)) = self.resolve(bind, loops) else {
            return vec![];
        };
        let scope: Vec<(String, usize)> = loops.iter().map(|l| (l.name.clone(), l.item)).collect();
        let Some(item) = self
            .shapes
            .find(bind, &scope)
            .and_then(|at| self.shapes.nodes[at].item)
        else {
            return vec![];
        };
        let reserved = swift::is_keyword(name) || VIEW_MEMBERS.contains(&name.as_str());
        let ident = if reserved {
            format!("{name}_")
        } else {
            name.clone()
        };
        let mut index = format!("{ident}Index");
        if self.loop_names.contains(&index) {
            index = format!("{ident}_index");
        }
        // The absolute data path of the iterated list: `$.todos` or, inside another loop,
        // that loop's item path plus the rest.
        let rest = bind.strip_prefix('$').unwrap_or_default();
        let (head, tail) = rest.split_once('.').unwrap_or((rest, ""));
        let base = if head.is_empty() {
            format!("$.{}", literal_content(tail))
        } else {
            let outer = loops.iter().rev().find(|l| l.name == head);
            let outer_path = outer.map(|l| l.item_path.clone()).unwrap_or_default();
            if tail.is_empty() {
                outer_path
            } else {
                format!("{outer_path}.{}", literal_content(tail))
            }
        };
        loops.push(Loop {
            name: name.clone(),
            ident: ident.clone(),
            item,
            write: format!("{write}[{index}]"),
            item_path: format!("{base}.\\({index})"),
        });
        let body = self.children(&node.children, loops, ctx, path);
        loops.pop();
        let id = node.id.clone().unwrap_or_default();
        let mut lines = vec![format!(
            "ForEach(Array({read}.enumerated()), id: \\.offset) {{ {index}, {ident} in"
        )];
        lines.extend(indent(body));
        lines.push("}".to_owned());
        let mut v = V::new(lines);
        v.modifier(format!(".weftEach({})", string_literal(&id)));
        v.render()
    }

    #[allow(clippy::too_many_arguments)]
    fn kind(
        &mut self,
        node: &Node,
        id: &str,
        props: &mut IndexMap<String, Value>,
        on: &mut IndexMap<String, String>,
        label: Option<&Value>,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> V {
        let label_text = label
            .map(|l| self.expr(l, Leaf::Text, loops))
            .unwrap_or_else(|| string_literal(""));
        match node.kind.as_str() {
            "screen" => {
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block("VStack(alignment: .leading)", body));
                v.container = true;
                v
            }
            "stack" => {
                let row = props.get("direction") == Some(&Value::String("row".to_owned()));
                if row {
                    props.shift_remove("direction");
                }
                let mut args = vec![];
                if let Some(Value::String(align)) = props.get("align") {
                    let a = match (row, align.as_str()) {
                        (false, "start") => Some(".leading"),
                        (false, "end") => Some(".trailing"),
                        (true, "start") => Some(".top"),
                        (true, "end") => Some(".bottom"),
                        (_, "center") => Some(".center"),
                        _ => None,
                    };
                    if let Some(a) = a {
                        args.push(format!("alignment: {a}"));
                        props.shift_remove("align");
                    }
                }
                if let Some(Value::Token(t)) = props.get("gap") {
                    args.push(format!("spacing: {}", token_expr(t)));
                    props.shift_remove("gap");
                }
                let name = if row { "HStack" } else { "VStack" };
                let head = if args.is_empty() {
                    name.to_owned()
                } else {
                    format!("{name}({})", args.join(", "))
                };
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block(&head, body));
                v.container = true;
                material(&mut v, props);
                v
            }
            "grid" => {
                let count = match props.shift_remove("columns") {
                    Some(Value::Number(n)) => swift::number_literal(n),
                    Some(other) => format!("weftColumns({})", self.expr(&other, Leaf::Int, loops)),
                    None => "1".to_owned(),
                };
                let head = match props.get("gap") {
                    Some(Value::Token(t)) => {
                        let s = token_expr(t);
                        format!(
                            "LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: {s}), count: {count}), spacing: {s})"
                        )
                    }
                    _ => format!(
                        "LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: {count}))"
                    ),
                };
                if matches!(props.get("gap"), Some(Value::Token(_))) {
                    props.shift_remove("gap");
                }
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block(&head, body));
                v.container = true;
                material(&mut v, props);
                v
            }
            "section" => {
                let body = self.children(&node.children, loops, ctx, path);
                let mut lines = block("Section", body);
                if let Some(header) = self.slot(node, "header", loops, ctx, path) {
                    lines = and_block(lines, "header", header);
                }
                let mut v = V::new(lines);
                v.container = true;
                v
            }
            "heading" => {
                let text = self.text_view(node, props, loops);
                let mut v = V::line(text);
                match props.shift_remove("level") {
                    Some(Value::Number(n)) => {
                        let level = n.round().clamp(1.0, 6.0) as usize;
                        v.modifier(format!(".font({})", HEADING_FONTS[level - 1]));
                        v.modifier(format!(".accessibilityHeading(.h{level})"));
                    }
                    Some(other) => {
                        let e = self.expr(&other, Leaf::Int, loops);
                        v.modifier(format!(".font(weftHeadingFont({e}))"));
                        v.modifier(format!(".accessibilityHeading(weftHeadingLevel({e}))"));
                    }
                    None => {}
                }
                v.modifier(".accessibilityAddTraits(.isHeader)");
                v
            }
            "text" => {
                let text = self.text_view(node, props, loops);
                let mut v = V::line(text);
                if let Some(Value::String(tone)) = props.get("tone") {
                    let style = match tone.as_str() {
                        "default" => Some(".primary"),
                        "muted" => Some(".secondary"),
                        "success" => Some(".green"),
                        "warning" => Some(".orange"),
                        "danger" => Some(".red"),
                        _ => None,
                    };
                    if let Some(style) = style {
                        v.modifier(format!(".foregroundStyle({style})"));
                        props.shift_remove("tone");
                    }
                }
                v
            }
            "image" => {
                let src = props
                    .shift_remove("src")
                    .map(|s| self.expr(&s, Leaf::Text, loops))
                    .unwrap_or_else(|| string_literal(""));
                V::line(format!("AsyncImage(url: weftURL({src}))"))
            }
            "model" => {
                // `src` (glTF) has no SwiftUI form and stays a `weftProp` marker.
                let mut path = |name: &str| {
                    props
                        .shift_remove(name)
                        .map(|v| self.expr(&v, Leaf::Text, loops))
                };
                let usdz = path("usdz").map(|usdz| format!("usdz: {usdz}, "));
                let fallback = path("fallback").unwrap_or_else(|| string_literal(""));
                V::line(format!(
                    "WeftModel({}fallback: {fallback})",
                    usdz.unwrap_or_default()
                ))
            }
            "link" => {
                let mut action = vec![];
                if let Some(href) = props.shift_remove("href") {
                    action.push(format!("openLink({})", self.expr(&href, Leaf::Text, loops)));
                }
                if let Some(a) = on.shift_remove("press") {
                    action.push(self.send(&a, id, loops));
                }
                let text = self.text_view(node, props, loops);
                let mut v = V::new(and_block(block("Button", action), "label", vec![text]));
                v.modifier(".buttonStyle(.borderless)");
                v.modifier(".accessibilityRemoveTraits(.isButton)");
                v.modifier(".accessibilityAddTraits(.isLink)");
                v
            }
            "button" => {
                let variant = match props.get("variant") {
                    Some(Value::String(s)) => Some(s.clone()),
                    _ => None,
                };
                let mut action = vec![];
                if let Some(a) = on.shift_remove("press") {
                    action.push(self.send(&a, id, loops));
                }
                if props.get("submit") == Some(&Value::Bool(true)) {
                    props.shift_remove("submit");
                    action.push(format!(
                        "submit({})",
                        string_literal(ctx.form.unwrap_or_default())
                    ));
                }
                let head = if variant.as_deref() == Some("danger") {
                    "Button(role: .destructive)"
                } else {
                    "Button"
                };
                let text = self.text_view(node, props, loops);
                let mut v = V::new(and_block(block(head, action), "label", vec![text]));
                let style = match variant.as_deref() {
                    Some("primary") | Some("danger") => Some(".borderedProminent"),
                    Some("secondary") => Some(".bordered"),
                    _ => None,
                };
                if let Some(style) = style {
                    v.modifier(format!(".buttonStyle({style})"));
                    props.shift_remove("variant");
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "form" => {
                let inner = Ctx { form: Some(id) };
                let body = self.children(&node.children, loops, inner, path);
                let lines = match self.slot(node, "footer", loops, inner, path) {
                    Some(footer) => {
                        block("Form", and_block(block("Section", body), "footer", footer))
                    }
                    None => block("Form", body),
                };
                let mut v = V::new(lines);
                if let Some(a) = on.shift_remove("submit") {
                    v.modifier(format!(".onSubmit {{ {} }}", self.send(&a, id, loops)));
                }
                v.container = true;
                v
            }
            "field" => {
                let kind = props.get("type").cloned();
                let (binding, blank) = self.binding(props.get("value"), Leaf::Text, loops);
                let read = self.read_writable(props.get("value"), Leaf::Text, loops);
                if !blank {
                    props.shift_remove("value");
                }
                let mut args = vec![label_text.clone(), format!("text: {binding}")];
                if let Some(p) = props.shift_remove("placeholder") {
                    args.push(format!(
                        "prompt: Text({})",
                        self.expr(&p, Leaf::Text, loops)
                    ));
                }
                let mut name = "TextField";
                let mut field_type = None;
                match &kind {
                    Some(Value::String(t)) if t == "password" => {
                        name = "SecureField";
                        props.shift_remove("type");
                    }
                    Some(Value::String(t)) if t == "multiline" => {
                        args.push("axis: .vertical".to_owned());
                        props.shift_remove("type");
                    }
                    Some(other) => {
                        field_type = Some(self.expr(other, Leaf::Text, loops));
                        props.shift_remove("type");
                    }
                    None => {}
                }
                let mut v = V::line(format!("{name}({})", args.join(", ")));
                v.captioned = true;
                if let Some(t) = field_type {
                    v.modifier(format!(".weftFieldType({t})"));
                }
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "checkbox" | "switch" => {
                let (binding, blank) = self.binding(props.get("checked"), Leaf::Bool, loops);
                let read = self.read_writable(props.get("checked"), Leaf::Bool, loops);
                if !blank {
                    props.shift_remove("checked");
                }
                let mut v = V::line(format!("Toggle({label_text}, isOn: {binding})"));
                v.captioned = true;
                v.modifier(if node.kind == "switch" {
                    ".toggleStyle(.switch)"
                } else {
                    ".toggleStyle(WeftCheckboxStyle())"
                });
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "slider" => {
                let (min, max, step) = self.number_props(props, loops);
                let value = props.get("value").cloned();
                let read = self.read_writable(value.as_ref(), Leaf::Double, loops);
                let binding = self.number_binding(props, loops);
                // `ClosedRange` traps when the bounds are reversed: a range is a literal only when
                // the document wrote both bounds, as the renderer reads them (a `max` below `min`
                // is `min`); otherwise the helper fills what is absent and applies the same rule.
                let range = match (&min, &max) {
                    (Num::Lit(_), Num::Lit(_)) => {
                        let span = controls::slider_span(min.lit(), max.lit(), None);
                        format!(
                            "{}...{}",
                            swift::number_literal(span.min),
                            swift::number_literal(span.max)
                        )
                    }
                    _ => format!("weftRange({}, {})", min.code("nil"), max.code("nil")),
                };
                let head = format!(
                    "Slider(value: {binding}, in: {range}, step: {})",
                    step_code(&step, true)
                );
                let mut v = V::new(block(&head, vec![format!("Text({label_text})")]));
                v.captioned = true;
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "stepper" => {
                let (min, max, step) = self.number_props(props, loops);
                let value = props.get("value").cloned();
                let read = self.read_writable(value.as_ref(), Leaf::Double, loops);
                let binding = self.number_binding(props, loops);
                let mut args = vec![label_text.clone(), format!("value: {binding}")];
                match (&min, &max) {
                    (Num::Absent, Num::Absent) => {}
                    (Num::Lit(_), Num::Lit(_)) => {
                        let b = controls::stepper_bounds(min.lit(), max.lit(), None);
                        args.push(format!(
                            "in: {}...{}",
                            swift::number_literal(b.min.unwrap_or(0.0)),
                            swift::number_literal(b.max.unwrap_or(0.0))
                        ));
                    }
                    // One open side has no range literal; the helper opens it.
                    _ => args.push(format!(
                        "in: weftBounds({}, {})",
                        min.code("nil"),
                        max.code("nil")
                    )),
                }
                if !matches!(step, Num::Absent) {
                    args.push(format!("step: {}", step_code(&step, false)));
                }
                let mut v = V::line(format!("Stepper({})", args.join(", ")));
                v.captioned = true;
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "date-picker" => {
                let kind = props.shift_remove("type");
                // An absent `type` is left out of the helper calls, so it reads back absent.
                let type_arg = kind
                    .as_ref()
                    .map(|t| format!(", {}", self.expr(t, Leaf::Text, loops)))
                    .unwrap_or_default();
                let type_text = type_arg.trim_start_matches(", ").to_owned();
                let components = match &kind {
                    Some(Value::String(t)) => match DateType::of(t) {
                        DateType::Date => "[.date]".to_owned(),
                        DateType::Time => "[.hourAndMinute]".to_owned(),
                        DateType::DateTime => "[.date, .hourAndMinute]".to_owned(),
                    },
                    None => "[.date]".to_owned(),
                    Some(_) => format!("weftComponents({type_text})"),
                };
                let (binding, blank) = self.binding(props.get("value"), Leaf::Text, loops);
                let read = self.read_writable(props.get("value"), Leaf::Text, loops);
                if !blank {
                    props.shift_remove("value");
                }
                let min = props.shift_remove("min");
                let max = props.shift_remove("max");
                let mut args = vec![
                    label_text.clone(),
                    format!("selection: weftDate({binding}{type_arg})"),
                ];
                if min.is_some() || max.is_some() {
                    let edge = |this: &Self, v: &Option<Value>| match v {
                        Some(v) => this.expr(v, Leaf::Text, loops),
                        None => string_literal(""),
                    };
                    args.push(format!(
                        "in: weftDates({}, {}{type_arg})",
                        edge(self, &min),
                        edge(self, &max)
                    ));
                }
                args.push(format!("displayedComponents: {components}"));
                let mut v = V::line(format!("DatePicker({})", args.join(", ")));
                v.captioned = true;
                // The text is a calendar date, not an instant: every device reads and shows it in
                // one zone, so a value means the same everywhere.
                v.modifier(".weftUTC()");
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "color-picker" => {
                let (binding, blank) = self.binding(props.get("value"), Leaf::Text, loops);
                let read = self.read_writable(props.get("value"), Leaf::Text, loops);
                if !blank {
                    props.shift_remove("value");
                }
                let mut v = V::line(format!(
                    "ColorPicker({label_text}, selection: weftColor({binding}), supportsOpacity: false)"
                ));
                v.captioned = true;
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "combobox" => {
                let (binding, blank) = self.binding(props.get("value"), Leaf::Text, loops);
                let read = self.read_writable(props.get("value"), Leaf::Text, loops);
                if !blank {
                    props.shift_remove("value");
                }
                let mut args = vec![label_text.clone(), format!("text: {binding}")];
                if let Some(p) = props.shift_remove("placeholder") {
                    args.push(format!(
                        "prompt: Text({})",
                        self.expr(&p, Leaf::Text, loops)
                    ));
                }
                let options = self.children(&node.children, loops, ctx, path);
                // SwiftUI has no combobox: a text field beside a menu of the options, which the
                // marker below reads back as one control.
                let picker = block(&format!("Picker(\"\", selection: {binding})"), options);
                let menu = and_block(
                    block("Menu", picker),
                    "label",
                    vec!["Image(systemName: \"chevron.down\")".to_owned()],
                );
                let mut body = vec![format!("TextField({})", args.join(", "))];
                body.extend(menu);
                let mut v = V::new(block("HStack", body));
                v.captioned = true;
                v.container = true;
                v.modifier(".weftCombobox()");
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "radio-group" | "segmented-control" | "select" => {
                let (binding, blank) = self.binding(props.get("value"), Leaf::Text, loops);
                let read = self.read_writable(props.get("value"), Leaf::Text, loops);
                if !blank {
                    props.shift_remove("value");
                }
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block(
                    &format!("Picker({label_text}, selection: {binding})"),
                    body,
                ));
                v.captioned = true;
                match node.kind.as_str() {
                    "radio-group" => v.modifier(".pickerStyle(.inline)"),
                    "segmented-control" => v.modifier(".pickerStyle(.segmented)"),
                    _ => {}
                }
                if let Some(a) = on.shift_remove("change") {
                    v.modifier(format!(
                        ".onChange(of: {read}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "radio" | "option" | "segment" => {
                let text = self.text_view(node, props, loops);
                let mut v = V::line(text);
                if let Some(value) = props.shift_remove("value") {
                    v.modifier(format!(".tag({})", self.expr(&value, Leaf::Text, loops)));
                }
                self.disabled(&mut v, props, loops);
                v
            }
            "list" | "table" => {
                let body = if node.kind == "table" {
                    self.table_body(node, loops, ctx, path)
                } else {
                    self.children(&node.children, loops, ctx, path)
                };
                let head = if node.kind == "table" {
                    "Grid(alignment: .leading)"
                } else {
                    "List"
                };
                let mut v = V::new(block(head, body));
                if let Some(empty) = self.slot(node, "empty", loops, ctx, path) {
                    let condition = self.empty_condition(node, props, loops);
                    let content = match condition {
                        Some(c) => block(&format!("if {c}"), empty),
                        None => empty,
                    };
                    v.mods.push(block(".overlay", content));
                }
                v.container = true;
                v
            }
            "item" | "cell" => {
                let body = self.mixed(node, props, loops, ctx, path);
                let mut v = V::new(block("HStack", body));
                self.press(&mut v, on, id, loops);
                v.container = true;
                v
            }
            "column" => {
                let text = self.text_view(node, props, loops);
                let mut v = V::line(text);
                v.modifier(".font(.headline)");
                self.press(&mut v, on, id, loops);
                v
            }
            "row" => {
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block("GridRow", body));
                self.press(&mut v, on, id, loops);
                v.container = true;
                v
            }
            "tabs" => {
                let body = self.children(&node.children, loops, ctx, path);
                let head = match props.get("selected") {
                    None => "TabView".to_owned(),
                    Some(value) => {
                        let (binding, _) = self.binding(Some(value), Leaf::Text, loops);
                        format!("TabView(selection: {binding})")
                    }
                };
                let read = self.read_writable(props.get("selected"), Leaf::Text, loops);
                let bound = props.shift_remove("selected").is_some();
                let mut v = V::new(block(&head, body));
                if let Some(a) = on.shift_remove("change") {
                    let of = if bound { read } else { string_literal("") };
                    v.modifier(format!(
                        ".onChange(of: {of}) {{ {} }}",
                        self.send(&a, id, loops)
                    ));
                }
                v
            }
            "tab" => {
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block("VStack(alignment: .leading)", body));
                v.captioned = true;
                v.mods
                    .push(vec![format!(".tabItem {{ Text({label_text}) }}")]);
                v.modifier(format!(".tag({})", string_literal(id)));
                v.container = true;
                v
            }
            "dialog" => {
                let (open, blank) = self.binding(props.get("open"), Leaf::Bool, loops);
                if !blank {
                    props.shift_remove("open");
                }
                let dismiss = match on.shift_remove("close") {
                    Some(a) => format!(", onDismiss: {{ {} }}", self.send(&a, id, loops)),
                    None => String::new(),
                };
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(block("VStack(alignment: .leading)", body));
                v.sheet = Some(format!(".sheet(isPresented: {open}{dismiss})"));
                if let Some(actions) = self.slot(node, "actions", loops, ctx, path) {
                    v.mods.push(block(
                        ".safeAreaInset(edge: .bottom)",
                        block("HStack", actions),
                    ));
                }
                if let Some(Value::Bool(modal)) = props.get("modal") {
                    let mode = if *modal { ".disabled" } else { ".enabled" };
                    v.modifier(format!(".presentationBackgroundInteraction({mode})"));
                    props.shift_remove("modal");
                }
                v.container = true;
                v
            }
            "alert" => {
                let body = self.mixed(node, props, loops, ctx, path);
                let mut v = V::new(block("GroupBox", body));
                if let Some(Value::String(tone)) = props.get("tone") {
                    let color = match tone.as_str() {
                        "info" => Some("blue"),
                        "success" => Some("green"),
                        "warning" => Some("orange"),
                        "danger" => Some("red"),
                        _ => None,
                    };
                    if let Some(color) = color {
                        v.modifier(format!(".backgroundStyle(Color.{color}.opacity(0.15))"));
                        props.shift_remove("tone");
                    }
                }
                v.container = true;
                v
            }
            "menu" => {
                let body = self.children(&node.children, loops, ctx, path);
                let mut v = V::new(and_block(
                    block("Menu", body),
                    "label",
                    vec![format!("Text({label_text})")],
                ));
                v.captioned = true;
                v
            }
            "menu-item" => {
                let mut action = vec![];
                if let Some(a) = on.shift_remove("press") {
                    action.push(self.send(&a, id, loops));
                }
                let text = self.text_view(node, props, loops);
                let mut v = V::new(and_block(block("Button", action), "label", vec![text]));
                self.disabled(&mut v, props, loops);
                v
            }
            other => match self.catalog.components.get(other) {
                Some(def) if !is_core_kind(other) => {
                    self.custom_view(node, def, id, props, on, loops, ctx, path)
                }
                _ => {
                    self.problems.push(Unsupported::new(
                        path,
                        format!("`{other}` has no SwiftUI form yet"),
                    ));
                    V::line("EmptyView()")
                }
            },
        }
    }

    /// A kind of the project's own catalog: a call of the view the app writes for it (README,
    /// "Custom components"). Props come in catalog order as labelled arguments, events as
    /// `on…` closures, then the content and every slot as view builders, so the view's
    /// signature never depends on what one screen happens to set.
    #[allow(clippy::too_many_arguments)]
    fn custom_view(
        &mut self,
        node: &Node,
        def: &ComponentDef,
        id: &str,
        props: &mut IndexMap<String, Value>,
        on: &mut IndexMap<String, String>,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> V {
        let view = swift::view_name(&node.kind);
        self.custom.insert(view.clone(), node.kind.clone());
        let mut labels: Vec<String> = vec![];
        let mut args = vec![];
        for (name, prop) in def.props.iter().flatten() {
            let Some(value) = props.shift_remove(name) else {
                continue;
            };
            let label = swift::label(name);
            labels.push(label.clone());
            let here = format!("{path}/@{name}");
            let expr = match (prop.kind, &value) {
                (PropType::Token, Value::Token(t)) => token_expr(t),
                // Shapes reports a bound token prop; nothing else reaches a token prop.
                (PropType::Token, _) => continue,
                (PropType::Number, _) if prop.writable == Some(true) => {
                    self.problems.push(Unsupported::new(
                        here,
                        "a writable number of a custom view is not generated",
                    ));
                    continue;
                }
                (PropType::Number, _) => {
                    let e = self.expr(&value, Leaf::Int, loops);
                    if prop.integer == Some(true) || !matches!(value, Value::Bind { .. }) {
                        e
                    } else {
                        format!("Double({e})")
                    }
                }
                (kind, _) => {
                    let leaf = if kind == PropType::Boolean {
                        Leaf::Bool
                    } else {
                        Leaf::Text
                    };
                    if prop.writable == Some(true) {
                        self.binding(Some(&value), leaf, loops).0
                    } else {
                        self.expr(&value, leaf, loops)
                    }
                }
            };
            args.push(format!("{label}: {expr}"));
        }
        if let Some(state) = props.shift_remove("state") {
            labels.push("state".to_owned());
            args.push(format!("state: {}", self.expr(&state, Leaf::Text, loops)));
        }
        for event in def.events.iter().flatten() {
            let Some(action) = on.shift_remove(event) else {
                continue;
            };
            let label = swift::event_label(event);
            labels.push(label.clone());
            args.push(format!("{label}: {{ {} }}", self.send(&action, id, loops)));
        }
        let mut closures: Vec<(String, Vec<String>)> = vec![];
        if def.content != Content::None {
            let body = self.children(&node.children, loops, ctx, path);
            closures.push(("content".to_owned(), body));
        }
        for name in def.slots.iter().flat_map(|s| s.keys()) {
            let body = self.slot(node, name, loops, ctx, path).unwrap_or_default();
            closures.push((swift::label(name), body));
        }
        labels.extend(closures.iter().map(|(l, _)| l.clone()));
        let mut seen = HashSet::new();
        if let Some(clash) = labels.iter().find(|l| !seen.insert(l.as_str())) {
            self.problems.push(Unsupported::new(
                path,
                format!(
                    "two names of `{}` become the Swift label `{clash}`",
                    node.kind
                ),
            ));
        }
        let head = match (args.is_empty(), closures.is_empty()) {
            (true, false) => view,
            _ => format!("{view}({})", args.join(", ")),
        };
        let mut lines = vec![head];
        for (i, (label, body)) in closures.into_iter().enumerate() {
            // An empty builder still names its slot, so the call keeps the view's signature.
            let body = if body.is_empty() {
                vec!["EmptyView()".to_owned()]
            } else {
                body
            };
            lines = if i == 0 {
                let first = lines.pop().unwrap_or_default();
                block(&first, body)
            } else {
                and_block(lines, &label, body)
            };
        }
        let mut v = V::new(lines);
        v.container = def.content != Content::None || def.slots.is_some();
        v
    }

    /// The `min`, `max` and `step` of a slider or stepper, taken out of `props`.
    fn number_props(&self, props: &mut IndexMap<String, Value>, loops: &[Loop]) -> (Num, Num, Num) {
        let mut take = |name: &str| self.number(props.shift_remove(name).as_ref(), loops);
        let min = take("min");
        let max = take("max");
        let step = take("step");
        (min, max, step)
    }

    /// The `Binding<Double>` of a slider or stepper. A literal is a constant; a zero reads the
    /// same as an absent value, so it keeps its marker, like blank text.
    fn number_binding(&self, props: &mut IndexMap<String, Value>, loops: &[Loop]) -> String {
        let (binding, _) = self.binding(props.get("value"), Leaf::Double, loops);
        let zero = matches!(self.number(props.get("value"), loops), Num::Lit(n) if n == 0.0);
        if !zero {
            props.shift_remove("value");
        }
        binding
    }

    /// A number prop of the number controls: absent, a literal, or a Swift expression.
    fn number(&self, value: Option<&Value>, loops: &[Loop]) -> Num {
        match value {
            None => Num::Absent,
            Some(Value::Number(n)) => {
                controls::numeric(Some(*n), None).map_or(Num::Absent, Num::Lit)
            }
            Some(Value::String(s)) => {
                controls::numeric(None, Some(s)).map_or(Num::Absent, Num::Lit)
            }
            Some(v @ Value::Bind { .. }) => Num::Expr(self.expr(v, Leaf::Double, loops)),
            Some(_) => Num::Absent,
        }
    }

    /// The value a writable prop currently holds, for `.onChange(of:)`.
    fn read_writable(&self, value: Option<&Value>, leaf: Leaf, loops: &[Loop]) -> String {
        match value {
            Some(v) => self.expr(v, leaf, loops),
            None => match leaf {
                Leaf::Bool => "false".to_owned(),
                Leaf::Int | Leaf::Double => "0".to_owned(),
                Leaf::Text => string_literal(""),
            },
        }
    }

    fn disabled(&self, v: &mut V, props: &mut IndexMap<String, Value>, loops: &[Loop]) {
        if let Some(d) = props.shift_remove("disabled") {
            v.modifier(format!(".disabled({})", self.expr(&d, Leaf::Bool, loops)));
        }
    }

    fn press(&self, v: &mut V, on: &mut IndexMap<String, String>, id: &str, loops: &[Loop]) {
        if let Some(a) = on.shift_remove("press") {
            v.modifier(format!(".onTapGesture {{ {} }}", self.send(&a, id, loops)));
        }
    }

    /// Content of a `mixed` kind: the `text` prop, or text runs and elements in order.
    fn mixed(
        &mut self,
        node: &Node,
        props: &mut IndexMap<String, Value>,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> Vec<String> {
        if props.contains_key("text") {
            return vec![self.text_view(node, props, loops)];
        }
        self.children(&node.children, loops, ctx, path)
    }

    /// Column headers in one unnamed `GridRow`, rows as they come.
    fn table_body(
        &mut self,
        node: &Node,
        loops: &mut Vec<Loop>,
        ctx: Ctx<'_>,
        path: &str,
    ) -> Vec<String> {
        let mut out = vec![];
        let mut header: Vec<String> = vec![];
        for child in &node.children {
            let Child::Node(n) = child else { continue };
            let is_column = n.kind == "column"
                || (n.kind == EACH
                    && n.children.iter().all(|c| match c {
                        Child::Node(c) => c.kind == "column",
                        Child::Text(_) => false,
                    }));
            let lines = self.node(n, loops, ctx, path);
            if is_column {
                header.extend(lines);
            } else {
                if !header.is_empty() {
                    out.extend(block("GridRow", std::mem::take(&mut header)));
                }
                out.extend(lines);
            }
        }
        if !header.is_empty() {
            out.extend(block("GridRow", header));
        }
        out
    }

    /// When a list or table shows its `empty` slot (SPEC §5.1 notes); `None` means always.
    fn empty_condition(
        &self,
        node: &Node,
        props: &IndexMap<String, Value>,
        loops: &[Loop],
    ) -> Option<String> {
        let mut eaches = vec![];
        let mut has_static = false;
        for child in &node.children {
            let Child::Node(n) = child else { continue };
            if n.kind == EACH {
                if let Some(Value::Bind { bind, .. }) = n.props.get("in")
                    && let Some((read, _, _)) = self.resolve(bind, loops)
                {
                    eaches.push(format!("{read}.isEmpty"));
                }
            } else if n.kind != "column" {
                has_static = true;
            }
        }
        let items = if has_static {
            Some("false".to_owned())
        } else if eaches.is_empty() {
            None
        } else {
            Some(eaches.join(" && "))
        };
        let state = match props.get("state") {
            Some(Value::String(s)) if s == "empty" => return None,
            Some(v @ Value::Bind { .. }) => {
                Some(format!("{} == \"empty\"", self.expr(v, Leaf::Text, loops)))
            }
            _ => None,
        };
        match (items, state) {
            (None, _) => None,
            (Some(i), None) => Some(i),
            (Some(i), Some(s)) if i == "false" => Some(s),
            (Some(i), Some(s)) => Some(format!("({i}) || {s}")),
        }
    }

    /// A dialog is a sheet presented from an invisible anchor in its place in the layout.
    fn dialog_anchor(&self, mut content: V, hidden: Option<&Value>, loops: &[Loop]) -> Vec<String> {
        let sheet = content.sheet.take().unwrap_or_default();
        let mut anchor = V::line("Color.clear");
        anchor.modifier(".frame(width: 0, height: 0)");
        anchor.mods.push(block(&sheet, content.render()));
        if let Some(h) = hidden {
            anchor.modifier(format!(".weftHidden({})", self.expr(h, Leaf::Bool, loops)));
        }
        anchor.render()
    }
}

/// The inside of a string literal for `s`, for literals that also interpolate.
fn literal_content(s: &str) -> String {
    let lit = string_literal(s);
    lit.strip_prefix('"')
        .and_then(|l| l.strip_suffix('"'))
        .unwrap_or_default()
        .to_owned()
}

fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => c.to_uppercase().chain(chars).collect(),
        _ => format!("Field{s}"),
    }
}

fn unique_type(base: &str, taken: &mut Vec<String>) -> String {
    let base = if swift::is_plain_identifier(base) {
        base.to_owned()
    } else {
        "FieldData".to_owned()
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.contains(&name) {
        name = format!("{base}{n}");
        n += 1;
    }
    taken.push(name.clone());
    name
}

const HEADING_FONTS: [&str; 6] = [
    ".largeTitle",
    ".title",
    ".title2",
    ".title3",
    ".headline",
    ".subheadline",
];

/// Fixed helpers every generated file carries, `fileprivate` so several screens can share a module.
const HELPERS: &str = include_str!("helpers.swift");

/// The `model` view, carried only by a file that has a model: it needs RealityKit, which the other
/// screens do not link.
const MODEL_HELPERS: &str = include_str!("model.swift");

/// SwiftUI takes a perspective relative to the view, where CSS takes pixels; a view of this many
/// points is the one the two agree on. `generate` prints `NOMINAL / px` so the importer can read
/// the pixels back exactly.
pub(crate) const TILT_NOMINAL_VIEW: f64 = 400.0;

/// The `rotation3DEffect` chain of a tilt, taking its props out of `props`. CSS applies
/// `rotateX rotateY rotateZ` right to left and SwiftUI applies modifiers in order, so the chain
/// runs z, y, x. Only the last effect carries the perspective, as in CSS the perspective is
/// applied once to the whole transform.
fn tilt_modifiers(props: &mut IndexMap<String, Value>) -> Vec<String> {
    let mut number = |name: &str| match props.get(name) {
        Some(Value::Number(n)) => {
            let n = *n;
            props.shift_remove(name);
            Some(n)
        }
        _ => None,
    };
    let perspective = number("perspective");
    let turns: Vec<(f64, &str)> = [
        ("rotate-z", "x: 0, y: 0, z: 1"),
        ("rotate-y", "x: 0, y: 1, z: 0"),
        ("rotate-x", "x: 1, y: 0, z: 0"),
    ]
    .into_iter()
    .filter_map(|(name, axis)| number(name).map(|degrees| (degrees, axis)))
    .collect();
    let projection = |last: bool| match perspective {
        Some(p) if last => format!(
            "{} / {}",
            swift::number_literal(TILT_NOMINAL_VIEW),
            swift::number_literal(p)
        ),
        _ => "0".to_owned(),
    };
    let effect = |degrees: f64, axis: &str, last: bool| {
        format!(
            ".rotation3DEffect(.degrees({}), axis: ({axis}), perspective: {})",
            swift::number_literal(degrees),
            projection(last)
        )
    };
    match (turns.as_slice(), perspective) {
        ([], None) => vec![],
        // A perspective with nothing to turn: a zero axis carries it.
        ([], Some(_)) => vec![effect(0.0, "x: 0, y: 0, z: 0", true)],
        (turns, _) => turns
            .iter()
            .enumerate()
            .map(|(i, (degrees, axis))| effect(*degrees, axis, i + 1 == turns.len()))
            .collect(),
    }
}
