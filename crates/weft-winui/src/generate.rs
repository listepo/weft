//! Weft → WinUI: one XAML `Page` per screen, with the canonical markup in a leading
//! `weft:source winui` comment so `import_winui` gives the document back. The mapping is SPEC §9,
//! "To WinUI", and `docs/winui-design.md`.

use indexmap::IndexMap;
use weft_catalog::{Token, token_types};
use weft_core::{
    Catalog, Child, Diagnostic, Document, Mode, Node, ValidateOptions, Value, has_errors,
    js_number, serialize, validate_document,
};
use weft_import::provenance;

use crate::Unsupported;
use crate::data::{Data, Leaf};
use crate::xaml::{Element, boolean, literal, type_name};

/// The source comment's options start with this word; `import_winui` checks it.
pub(crate) const TARGET: &str = "winui";

/// The props each kind of this slice draws, besides `label`, `hidden` and `state`; every other
/// kind and prop is refused, never dropped.
const KINDS: &[(&str, &str, &str)] = &[
    ("screen", "StackPanel", ""),
    ("stack", "StackPanel", "direction gap justify wrap"),
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
const COMMON: &str = "label hidden state";

pub struct GenerateOptions<'a> {
    pub catalog: &'a Catalog,
    /// Resolved design tokens (`weft_catalog::load_tokens`), against which the document is checked.
    pub tokens: &'a IndexMap<String, Token>,
    /// The page is `<Name>Page`; from the screen id when absent.
    pub name: Option<&'a str>,
    /// The C# namespace of the page and of the `Weft` helper classes, such as `Acme.Screens`.
    pub namespace: &'a str,
}

/// The files of one screen. The view model and the code-behind follow (T105.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    /// The page class, `<Name>Page`, which is also the file name before `.xaml`.
    pub page: String,
    pub xaml: String,
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
    let identifier = |s: &str| {
        s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if !namespace.split('.').all(identifier) {
        return Err(GenerateError::Namespace(namespace.to_owned()));
    }
    // The comment keeps the screen as written (SPEC §9, Provenance); the page draws its uses.
    let source = serialize(document);
    let expanded = weft_core::expand(document, options.catalog).document;
    let name = type_name(
        options
            .name
            .or(expanded.root.id.as_deref())
            .unwrap_or("screen"),
    );
    let mut g = Gen::default();
    let panel = g.node(&expanded.root);
    let mut commands = vec![];
    actions(&expanded.root, &mut commands);
    g.data.check(&commands, &mut g.problems);
    if !g.problems.is_empty() {
        return Err(GenerateError::Unsupported(g.problems));
    }
    let page = format!("{name}Page");
    let mut root = Element::new("Page");
    root.attr("x:Class", format!("{namespace}.{page}"));
    root.attr(
        "xmlns",
        "http://schemas.microsoft.com/winfx/2006/xaml/presentation",
    );
    root.attr("xmlns:x", "http://schemas.microsoft.com/winfx/2006/xaml");
    root.attr("xmlns:weft", format!("using:{namespace}"));
    let mut scroll = Element::new("ScrollViewer");
    scroll.children.extend(panel);
    root.children.push(scroll);
    let options = format!("{TARGET} name={name} namespace={namespace}");
    let mut xaml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}\n",
        provenance::comment("<!--", &options, &source, "-->")
    );
    root.write(0, &mut xaml);
    Ok(Generated { page, xaml })
}

fn path_of(node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{}#{id}", node.kind),
        None => node.kind.clone(),
    }
}

/// `todo.add` → `todo_addCommand`: action names hold no `_` (SPEC §2.2), so the name maps back.
fn command(action: &str) -> String {
    format!("{}Command", action.replace('.', "_"))
}

fn actions(node: &Node, out: &mut Vec<String>) {
    out.extend(node.on.values().map(|a| command(a)));
    let slots = node.slots.values().flatten();
    for child in node.children.iter().chain(slots) {
        if let Child::Node(n) = child {
            actions(n, out);
        }
    }
}

#[derive(Default)]
struct Gen {
    data: Data,
    problems: Vec<Unsupported>,
    /// The enclosing forms' submit commands, innermost last.
    forms: Vec<Option<String>>,
}

impl Gen {
    fn refuse(&mut self, node: &Node, message: impl Into<String>) {
        self.problems.push(Unsupported::new(path_of(node), message));
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
            Value::Bind { bind, not: false } => self.bound(&at, bind, Some(leaf), two_way),
            // A negated binding is never two-way (SPEC §5.1), and x:Bind has no operators.
            Value::Bind { bind, not: true } if leaf == Leaf::Flag => self.call(&at, "Not", bind),
            _ => {
                self.refuse(node, format!("`{prop}` has no WinUI form here"));
                None
            }
        }
    }

    /// `{x:Bind ViewModel.path}` read as `leaf` (`None`: any type, for a marker).
    fn bound(&mut self, at: &str, bind: &str, leaf: Option<Leaf>, two_way: bool) -> Option<String> {
        let path = self.data.read(bind, leaf, at, &mut self.problems)?;
        let mode = if two_way { "TwoWay" } else { "OneWay" };
        Some(format!("{{x:Bind {path}, Mode={mode}}}"))
    }

    /// `{x:Bind weft:Weft.<function>(ViewModel.path)}`: a helper that takes any value.
    fn call(&mut self, at: &str, function: &str, bind: &str) -> Option<String> {
        let path = self.data.read(bind, None, at, &mut self.problems)?;
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
            Some(Value::Bind { bind, not: false }) => self.bound(&at, bind, None, false),
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
                && let Some(c) = self.node(n)
            {
                e.children.push(c);
            }
        }
    }

    fn node(&mut self, node: &Node) -> Option<Element> {
        let Some(&(kind, control, props)) = KINDS.iter().find(|k| k.0 == node.kind) else {
            self.later(node, &format!("`{}`", node.kind));
            return None;
        };
        let known = |p: &str| props.split(' ').chain(COMMON.split(' ')).any(|k| k == p);
        for prop in node.props.keys().filter(|p| !known(p)) {
            self.later(node, &format!("`{prop}`"));
        }
        for slot in node
            .slots
            .keys()
            .filter(|s| (kind, s.as_str()) != ("form", "footer"))
        {
            self.later(node, &format!("the `{slot}` slot"));
        }
        let password = kind == "field"
            && matches!(node.props.get("type"), Some(Value::String(s)) if s == "password");
        let mut e = Element::new(if password { "PasswordBox" } else { control });
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
            _ => {
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
        }
        let label = match kind {
            "field" | "switch" => "Header",
            "checkbox" => "Content",
            _ => "AutomationProperties.Name",
        };
        self.set(&mut e, node, "label", label, Leaf::Text);
        let visibility = match node.props.get("hidden") {
            Some(Value::Bool(b)) => Some(if *b { "Collapsed" } else { "Visible" }.to_owned()),
            Some(Value::Bind { bind, not: false }) => self.call(&path_of(node), "Hide", bind),
            // x:Bind turns a `bool` property into a `Visibility` itself.
            Some(Value::Bind { bind, not: true }) => {
                self.bound(&path_of(node), bind, Some(Leaf::Flag), false)
            }
            _ => None,
        };
        if let Some(v) = visibility {
            e.attr("Visibility", v);
        }
        self.marker(&mut e, node, "state", "State");
        for (event, action) in &node.on {
            let name = format!("weft:On.{}", type_name(event));
            e.attr(name, format!("{{x:Bind ViewModel.{}}}", command(action)));
        }
        if matches!(kind, "screen" | "stack" | "form") {
            self.children(&mut e, &node.children);
        }
        if let Some(footer) = node.slots.get("footer").filter(|_| kind == "form") {
            let mut row = Element::new("StackPanel");
            row.attr("Orientation", "Horizontal");
            row.attr("weft:Slot.Name", "footer");
            self.children(&mut row, footer);
            e.children.push(row);
        }
        if kind == "form" {
            self.forms.pop();
        }
        Some(e)
    }

    fn stack(&mut self, e: &mut Element, node: &Node) {
        let row = self.word(node, "direction") == Some("row");
        e.attr("Orientation", if row { "Horizontal" } else { "Vertical" });
        match node.props.get("gap") {
            None => {}
            Some(Value::Token(path)) => {
                e.attr("Spacing", format!("{{StaticResource Weft.{path}}}"))
            }
            Some(_) => self.refuse(node, "a `gap` that is not a token"),
        }
        if self.word(node, "justify").is_some_and(|j| j != "start") {
            self.later(node, "`justify` other than `start`");
        }
        self.marker(e, node, "wrap", "Wrap");
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
            e.attr("weft:On.Submit", format!("{{x:Bind ViewModel.{submit}}}"));
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
