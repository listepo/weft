//! Weft → WinUI: one XAML `Page` per screen, with the canonical markup in a leading
//! `weft:source winui` comment so `import_winui` gives the document back. The mapping is SPEC §9,
//! "To WinUI", and `docs/winui-design.md`.

use indexmap::IndexMap;
use weft_catalog::{Appearance, Token, token_types};
use weft_core::{
    Catalog, Child, Diagnostic, Document, Mode, Node, PropType, ValidateOptions, Value, has_errors,
    js_number, serialize, validate_document,
};
use weft_import::provenance;

use crate::Unsupported;
use crate::csharp::{self, command};
use crate::data::{Data, Leaf, Read};
use crate::xaml::{Element, boolean, literal, type_name};
use repetition::holder;

pub(crate) mod containers;
mod controls;
mod layout;
mod repetition;
pub(crate) mod surface;

/// The source comment's options start with this word; `import_winui` checks it.
pub(crate) const TARGET: &str = "winui";

/// The props each kind of this slice draws, besides `label`, `hidden` and `state`; every other
/// kind and prop is refused, never dropped.
const KINDS: &[(&str, &str, &str)] = &[
    ("screen", "StackPanel", ""),
    (
        "stack",
        "StackPanel",
        "direction gap align justify wrap padding max-width material",
    ),
    ("form", "StackPanel", ""),
    ("text", "TextBlock", "text tone"),
    ("heading", "TextBlock", "level text"),
    ("button", "Button", "text variant disabled submit"),
    (
        "field",
        "TextBox",
        "type value placeholder required disabled error",
    ),
    ("checkbox", "CheckBox", "checked disabled"),
    ("switch", "ToggleSwitch", "checked disabled"),
];
// Validation keeps `grow` to a stack's children, whose tracks draw it.
const COMMON: &str = "label hidden state grow rotate-x rotate-y rotate-z perspective";
/// The named slots of this slice's kinds; the containers add theirs.
const SLOTS: &[(&str, &str)] = &[("form", "footer")];

pub struct GenerateOptions<'a> {
    pub catalog: &'a Catalog,
    /// Resolved design tokens (`weft_catalog::load_tokens`), against which the document is checked.
    pub tokens: &'a IndexMap<String, Token>,
    /// The page is `<Name>Page`; from the screen id when absent.
    pub name: Option<&'a str>,
    /// The C# namespace of the page and of the `Weft` helper classes, such as `Acme.Screens`.
    pub namespace: &'a str,
    /// The page reads its tokens from the project's shared `WeftTokens.xaml` (`generate_tokens`)
    /// instead of holding the ones it uses in its `Page.Resources`, which lets it build alone.
    pub shared_tokens: bool,
    /// The light and dark tokens of the project's resolver (`weft_catalog::appearance`), for the
    /// `Dark` dictionary of a page's own tokens. The shared tokens take it in `generate_tokens`.
    pub appearance: Option<Appearance<'a>>,
}

/// The files of one screen (`files` names them). The page binds to its view model, and the
/// view model and the helpers of `helpers` are plain C#.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The page class, `<Name>Page`: the file name before `.xaml` and `.xaml.cs`.
    pub page: String,
    pub xaml: String,
    /// `<Name>Page.xaml.cs`: the page class with its `ViewModel`.
    pub code_behind: String,
    /// The view-model class, `<Name>ViewModel`: the file name before `.cs`.
    pub view_model: String,
    /// `<Name>ViewModel.cs`: one property per data path, one command per action, `Perform` and
    /// `Performed`.
    pub view_model_source: String,
}

impl Generated {
    /// Every file of the screen, as file name and content, in the order they are written: the
    /// CLI writes this list, so a new file needs no change there.
    pub fn files(&self) -> [(String, &str); 3] {
        [
            (format!("{}.xaml", self.page), &self.xaml),
            (format!("{}.xaml.cs", self.page), &self.code_behind),
            (format!("{}.cs", self.view_model), &self.view_model_source),
        ]
    }
}

/// `Weft.cs`, the helpers every page of `namespace` uses: the binding functions, the `Props`
/// markers, the `On` events, `Slot`, and the event and command types. A project writes it once.
pub fn helpers(namespace: &str) -> Result<String, GenerateError> {
    check_namespace(namespace)?;
    let helpers = include_str!("Weft.cs");
    Ok(helpers.replacen(
        "namespace WeftNamespace;",
        &format!("namespace {namespace};"),
        1,
    ))
}

fn check_namespace(namespace: &str) -> Result<(), GenerateError> {
    let identifier = |s: &str| {
        s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if namespace.split('.').all(identifier) {
        Ok(())
    } else {
        Err(GenerateError::Namespace(namespace.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    /// The document does not validate in strict mode against the catalog and tokens.
    #[error("the document has {} diagnostics", .0.len())]
    Invalid(Vec<Diagnostic>),
    #[error("`{0}` is not a C# namespace")]
    Namespace(String),
    /// The document is valid, but says something the generator cannot express in WinUI.
    #[error("cannot generate WinUI: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Unsupported(Vec<Unsupported>),
}

pub fn generate(
    document: &Document,
    options: &GenerateOptions<'_>,
) -> Result<Generated, GenerateError> {
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
    let namespace = options.namespace;
    check_namespace(namespace)?;
    // The comment keeps the screen as written (SPEC §9, Provenance); the page draws its uses.
    let source = serialize(document);
    let expanded = weft_core::expand(document, options.catalog).document;
    let name = type_name(
        options
            .name
            .or(expanded.root.id.as_deref())
            .unwrap_or("screen"),
    );
    let mut g = Gen {
        catalog: options.catalog,
        data: Data::default(),
        problems: vec![],
        forms: vec![],
        editable: false,
        columns: 0,
        name: name.clone(),
        used: IndexMap::new(),
    };
    let panel = g.node(&expanded.root);
    let mut names = vec![];
    actions(&expanded.root, &mut names);
    let commands: Vec<String> = names.iter().map(|a| command(a)).collect();
    g.data.check(&commands, &mut g.problems);
    let resources = (!options.shared_tokens).then(|| page_tokens(&mut g, options));
    if !g.problems.is_empty() {
        return Err(GenerateError::Unsupported(g.problems));
    }
    let view_model_source = csharp::view_model(&g.data, &name, namespace, &names)
        .map_err(GenerateError::Unsupported)?;
    let page = format!("{name}Page");
    let mut root = Element::new("Page");
    root.attr("x:Class", format!("{namespace}.{page}"));
    root.attr(
        "xmlns",
        "http://schemas.microsoft.com/winfx/2006/xaml/presentation",
    );
    root.attr("xmlns:x", "http://schemas.microsoft.com/winfx/2006/xaml");
    root.attr("xmlns:weft", format!("using:{namespace}"));
    if let Some(resources) = resources.filter(|r| !r.children.is_empty()) {
        root.children.push(holder("Page.Resources", resources));
    }
    let mut scroll = Element::new("ScrollViewer");
    scroll.children.extend(panel);
    root.children.push(scroll);
    let options = format!("{TARGET} name={name} namespace={namespace}");
    let mut xaml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}\n",
        provenance::comment("<!--", &options, &source, "-->")
    );
    root.write(0, &mut xaml);
    Ok(Generated {
        page,
        xaml,
        code_behind: csharp::code_behind(&name, namespace),
        view_model: format!("{name}ViewModel"),
        view_model_source,
    })
}

/// The page's own `ResourceDictionary`: the tokens it references, in the project's order. A
/// token WinUI has no form for would leave a reference that fails when the page loads.
fn page_tokens(g: &mut Gen<'_>, options: &GenerateOptions<'_>) -> Element {
    let used = options
        .tokens
        .iter()
        .filter(|(path, _)| g.used.contains_key(*path));
    let (entries, skipped) = crate::tokens::resources(used, options.appearance);
    for (path, why) in skipped {
        let at = g.used.get(&path).cloned().unwrap_or_default();
        g.problems
            .push(Unsupported::new(at, format!("token `{path}`: {why}")));
    }
    let mut dictionary = Element::new("ResourceDictionary");
    dictionary.children = entries;
    dictionary
}

fn path_of(node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{}#{id}", node.kind),
        None => node.kind.clone(),
    }
}

/// The actions the screen names, each once, in document order.
fn actions(node: &Node, out: &mut Vec<String>) {
    for action in node.on.values() {
        if !out.contains(action) {
            out.push(action.clone());
        }
    }
    let slots = node.slots.values().flatten();
    for child in node.children.iter().chain(slots) {
        if let Child::Node(n) = child {
            actions(n, out);
        }
    }
}

struct Gen<'a> {
    catalog: &'a Catalog,
    data: Data,
    problems: Vec<Unsupported>,
    /// The enclosing forms' submit commands, innermost last.
    forms: Vec<Option<String>>,
    /// Inside a `combobox`, whose options show their value.
    editable: bool,
    /// The columns of the enclosing table, which each of its rows lines up with.
    columns: usize,
    /// The page's name, which the generated classes start with.
    name: String,
    /// The tokens the page references, each with the first element that does.
    used: IndexMap<String, String>,
}

impl Gen<'_> {
    fn refuse(&mut self, node: &Node, message: impl Into<String>) {
        self.problems.push(Unsupported::new(path_of(node), message));
    }

    /// The markup `extension` reading a token's resource, `Weft.<path><suffix>`, noted for the
    /// page's own tokens.
    fn token(&mut self, node: &Node, extension: &str, path: &str, suffix: &str) -> String {
        if !self.used.contains_key(path) {
            self.used.insert(path.to_owned(), path_of(node));
        }
        format!("{{{extension} Weft.{path}{suffix}}}")
    }

    fn later(&mut self, node: &Node, what: &str) {
        self.refuse(node, format!("{what} is not generated for WinUI yet"));
    }

    /// A prop as a literal or a binding of type `leaf`; a writable one binds two-way.
    fn value(&mut self, node: &Node, prop: &str, leaf: Leaf, two_way: bool) -> Option<String> {
        let at = path_of(node);
        match node.props.get(prop)? {
            Value::String(s) if leaf == Leaf::Text => Some(literal(s)),
            Value::Bool(b) if leaf == Leaf::Flag => Some(boolean(*b).to_owned()),
            Value::Number(n) if leaf == Leaf::Number => Some(js_number(*n)),
            Value::Bind { bind, not: false } => self.bound(&at, bind, Read::Is(leaf), two_way),
            // A negated binding is never two-way (SPEC §5.1), and x:Bind has no operators.
            Value::Bind { bind, not: true } if leaf == Leaf::Flag => self.call(&at, "Not", bind),
            _ => {
                self.refuse(node, format!("`{prop}` has no WinUI form here"));
                None
            }
        }
    }

    /// `{x:Bind ViewModel.path}`.
    fn bound(&mut self, at: &str, bind: &str, read: Read, two_way: bool) -> Option<String> {
        let path = self.data.read(bind, read, at, &mut self.problems)?;
        let mode = if two_way { "TwoWay" } else { "OneWay" };
        Some(format!("{{x:Bind {path}, Mode={mode}}}"))
    }

    /// `{x:Bind weft:Weft.<function>(ViewModel.path)}`: a helper that tests any value as a flag.
    fn call(&mut self, at: &str, function: &str, bind: &str) -> Option<String> {
        let path = self
            .data
            .read(bind, Read::Like(Leaf::Flag), at, &mut self.problems)?;
        Some(format!(
            "{{x:Bind weft:Weft.{function}({path}), Mode=OneWay}}"
        ))
    }

    /// An inert `weft:Props.<name>` marker holding what WinUI does not draw (design, Markers).
    fn marker(&mut self, e: &mut Element, node: &Node, prop: &str, name: &str) {
        let at = path_of(node);
        let value = match node.props.get(prop) {
            None => return,
            Some(Value::String(s)) => Some(literal(s)),
            Some(Value::Bool(b)) => Some(boolean(*b).to_owned()),
            Some(Value::Number(n)) => Some(js_number(*n)),
            Some(Value::Bind { bind, not: false }) => {
                // A marker holds any value; the prop's catalog type suggests the property's.
                let def = self.catalog.components.get(&node.kind);
                let flag = def
                    .and_then(|d| d.prop(prop))
                    .is_some_and(|p| p.kind == PropType::Boolean);
                let leaf = if flag { Leaf::Flag } else { Leaf::Text };
                self.bound(&at, bind, Read::Like(leaf), false)
            }
            Some(Value::Bind { bind, not: true }) => self.call(&at, "Not", bind),
            Some(Value::Token(_)) => {
                self.refuse(node, format!("a token in `{prop}`"));
                None
            }
        };
        if let Some(v) = value {
            e.attr(format!("weft:Props.{name}"), v);
        }
    }

    fn set(&mut self, e: &mut Element, node: &Node, prop: &str, xaml: &str, leaf: Leaf) {
        if let Some(v) = self.value(node, prop, leaf, false) {
            e.attr(xaml, v);
        }
    }

    /// A literal-only enum prop: `None` when absent, refused when bound.
    fn word<'n>(&mut self, node: &'n Node, prop: &str) -> Option<&'n str> {
        match node.props.get(prop)? {
            Value::String(s) => Some(s),
            _ => {
                self.refuse(node, format!("a bound `{prop}`"));
                None
            }
        }
    }

    /// The text of a `text`-content kind: its `text` prop, or its content.
    fn content(&mut self, e: &mut Element, node: &Node, xaml: &str) {
        if node.props.contains_key("text") {
            return self.set(e, node, "text", xaml, Leaf::Text);
        }
        let text: String = node
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect();
        if !text.is_empty() {
            e.attr(xaml, literal(&text));
        }
    }

    fn children(&mut self, e: &mut Element, children: &[Child]) {
        for child in children {
            if let Child::Node(n) = child
                && let Some(c) = self.child(n, Some(e))
            {
                e.children.push(c);
            }
        }
    }

    fn node(&mut self, node: &Node) -> Option<Element> {
        if node.kind == "each" {
            return self.child(node, None);
        }
        let mut all = KINDS.iter().chain(controls::KINDS).chain(containers::KINDS);
        let Some(&(kind, control, props)) = all.find(|k| k.0 == node.kind) else {
            self.later(node, &format!("`{}`", node.kind));
            return None;
        };
        let known = |p: &str| props.split(' ').chain(COMMON.split(' ')).any(|k| k == p);
        for prop in node.props.keys().filter(|p| !known(p)) {
            self.later(node, &format!("`{prop}`"));
        }
        let drawn = |s: &&String| {
            let mut all = SLOTS.iter().chain(containers::SLOTS);
            all.any(|&(k, n)| (k, n) == (kind, s.as_str()))
        };
        for slot in node.slots.keys().filter(|s| !drawn(s)) {
            self.later(node, &format!("the `{slot}` slot"));
        }
        let password = kind == "field"
            && matches!(node.props.get("type"), Some(Value::String(s)) if s == "password");
        let mut e = Element::new(if password {
            "PasswordBox"
        } else {
            controls::element(node, control)
        });
        if let Some(id) = &node.id {
            e.attr("AutomationProperties.AutomationId", literal(id));
        }
        match kind {
            "screen" => e.attr("AutomationProperties.LandmarkType", "Main"),
            "stack" => self.stack(&mut e, node),
            "form" => {
                e.attr("AutomationProperties.LandmarkType", "Form");
                self.forms.push(node.on.get("submit").map(|a| command(a)));
            }
            "text" => {
                e.attr("TextWrapping", "Wrap");
                self.content(&mut e, node, "Text");
                let brush = match self.word(node, "tone") {
                    Some("muted") => Some("TextFillColorSecondaryBrush"),
                    Some("success") => Some("SystemFillColorSuccessBrush"),
                    Some("warning") => Some("SystemFillColorCautionBrush"),
                    Some("danger") => Some("SystemFillColorCriticalBrush"),
                    _ => None,
                };
                if let Some(brush) = brush {
                    e.attr("Foreground", format!("{{ThemeResource {brush}}}"));
                }
            }
            "heading" => self.heading(&mut e, node),
            "button" => self.button(&mut e, node),
            "field" => self.field(&mut e, node, password),
            _ if containers::KINDS.iter().any(|k| k.0 == kind) => self.container(&mut e, node),
            "checkbox" | "switch" => {
                if let Some(v) = self.value(node, "checked", Leaf::Flag, true) {
                    e.attr(
                        if kind == "switch" {
                            "IsOn"
                        } else {
                            "IsChecked"
                        },
                        v,
                    );
                }
                self.disabled(&mut e, node);
            }
            _ => self.control(&mut e, node),
        }
        let label = match kind {
            "field" | "switch" | "tab" => "Header",
            "checkbox" => "Content",
            _ => controls::caption(node).unwrap_or("AutomationProperties.Name"),
        };
        self.set(&mut e, node, "label", label, Leaf::Text);
        let visibility = match node.props.get("hidden") {
            Some(Value::Bool(b)) => Some(if *b { "Collapsed" } else { "Visible" }.to_owned()),
            Some(Value::Bind { bind, not: false }) => self.call(&path_of(node), "Hide", bind),
            // x:Bind turns a `bool` property into a `Visibility` itself.
            Some(Value::Bind { bind, not: true }) => {
                self.bound(&path_of(node), bind, Read::Is(Leaf::Flag), false)
            }
            _ => None,
        };
        if let Some(v) = visibility {
            e.attr("Visibility", v);
        }
        self.marker(&mut e, node, "state", "State");
        for (event, action) in &node.on {
            let name = format!("weft:On.{}", type_name(event));
            let root = self.data.root();
            e.attr(name, format!("{{x:Bind {root}.{}}}", command(action)));
        }
        if matches!(kind, "screen" | "form") {
            self.children(&mut e, &node.children);
        }
        if let Some(footer) = node.slots.get("footer").filter(|_| kind == "form") {
            let row = self.slot("footer", footer, true);
            e.children.push(row);
        }
        if kind == "form" {
            self.forms.pop();
        }
        Some(self.surface(e, node))
    }

    /// A `gap` token as each of the panel's spacing properties.
    fn gap(&mut self, e: &mut Element, node: &Node, attrs: &[&str]) {
        match node.props.get("gap") {
            None => {}
            Some(Value::Token(path)) => {
                for attr in attrs {
                    let value = self.token(node, "StaticResource", path, "");
                    e.attr(*attr, value);
                }
            }
            Some(_) => self.refuse(node, "a `gap` that is not a token"),
        }
    }

    fn heading(&mut self, e: &mut Element, node: &Node) {
        let level = match node.props.get("level") {
            Some(Value::Number(n)) if (1.0..=6.0).contains(n) && n.fract() == 0.0 => *n as usize,
            _ => {
                self.refuse(node, "`level` must be a literal from 1 to 6");
                1
            }
        };
        let style = ["TitleLarge", "Title", "Subtitle"]
            .get(level - 1)
            .unwrap_or(&"BodyStrong");
        e.attr("Style", format!("{{StaticResource {style}TextBlockStyle}}"));
        e.attr("AutomationProperties.HeadingLevel", format!("Level{level}"));
        self.content(e, node, "Text");
    }

    fn button(&mut self, e: &mut Element, node: &Node) {
        self.content(e, node, "Content");
        match self.word(node, "variant") {
            Some("primary") => e.attr("Style", "{StaticResource AccentButtonStyle}"),
            Some("danger") => self.marker(e, node, "variant", "Variant"),
            _ => {}
        }
        self.disabled(e, node);
        if matches!(node.props.get("submit"), Some(Value::Bool(true)))
            && let Some(Some(submit)) = self.forms.last()
        {
            let root = self.data.root();
            e.attr("weft:On.Submit", format!("{{x:Bind {root}.{submit}}}"));
        }
    }

    fn field(&mut self, e: &mut Element, node: &Node, password: bool) {
        let (text, trigger) = if password {
            ("Password", "")
        } else {
            // TextBox.Text writes back on focus loss by default; Weft fields write on every edit.
            ("Text", ", UpdateSourceTrigger=PropertyChanged")
        };
        if let Some(v) = self.value(node, "value", Leaf::Text, true) {
            e.attr(
                text,
                v.replace("Mode=TwoWay}", &format!("Mode=TwoWay{trigger}}}")),
            );
        }
        self.set(e, node, "placeholder", "PlaceholderText", Leaf::Text);
        match self.word(node, "type") {
            Some("email") => e.attr("InputScope", "EmailSmtpAddress"),
            Some("number") => e.attr("InputScope", "Number"),
            Some("search") => e.attr("InputScope", "Search"),
            Some("multiline") => {
                e.attr("AcceptsReturn", "True");
                e.attr("TextWrapping", "Wrap");
            }
            _ => {}
        }
        let required = "AutomationProperties.IsRequiredForForm";
        self.set(e, node, "required", required, Leaf::Flag);
        self.disabled(e, node);
        self.marker(e, node, "error", "Error");
    }

    /// `disabled` is `IsEnabled`, the opposite sense, so a binding goes through a helper.
    fn disabled(&mut self, e: &mut Element, node: &Node) {
        let at = path_of(node);
        let enabled = match node.props.get("disabled") {
            Some(Value::Bool(b)) => Some(boolean(!b).to_owned()),
            Some(Value::Bind { bind, not: false }) => self.call(&at, "Not", bind),
            Some(Value::Bind { bind, not: true }) => self.call(&at, "Truthy", bind),
            _ => None,
        };
        if let Some(v) = enabled {
            e.attr("IsEnabled", v);
        }
    }
}
