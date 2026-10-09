//! The shared data contracts of SPEC.md §3, §5 and §7: documents, nodes, values and catalogs.
//! Kept apart from the code that reads them so that every layer agrees on one shape.

use indexmap::IndexMap;
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};

use crate::source::Source;

pub const WEFT_VERSION: &str = "0.2";

/// Objects keep their key order, as JavaScript objects do.
pub type Map<V> = IndexMap<String, V>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Bind { bind: String, not: bool },
    Token(String),
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::String(v) => s.serialize_str(v),
            Value::Number(v) => s.serialize_f64(*v),
            Value::Bool(v) => s.serialize_bool(*v),
            Value::Bind { bind, not } => {
                let mut m = s.serialize_map(Some(if *not { 2 } else { 1 }))?;
                m.serialize_entry("bind", bind)?;
                if *not {
                    m.serialize_entry("not", &true)?;
                }
                m.end()
            }
            Value::Token(token) => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("token", token)?;
                m.end()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Child {
    Node(Box<Node>),
    Text(String),
}

impl Child {
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Child::Node(n) => Some(n),
            Child::Text(_) => None,
        }
    }
}

impl Serialize for Child {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Child::Node(n) => n.serialize(s),
            Child::Text(t) => s.serialize_str(t),
        }
    }
}

/// Empty members mean absent: canonical JSON omits them and no rule tells the two apart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
    pub kind: String,
    pub id: Option<String>,
    pub props: Map<Value>,
    pub on: Map<String>,
    pub slots: Map<Vec<Child>>,
    pub children: Vec<Child>,
    /// Markup positions; never serialized and ignored by equality.
    pub source: Source,
}

impl Node {
    pub fn new(kind: impl Into<String>) -> Self {
        Node {
            kind: kind.into(),
            ..Node::default()
        }
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Node", 6)?;
        st.serialize_field("kind", &self.kind)?;
        if let Some(id) = &self.id {
            st.serialize_field("id", id)?;
        }
        if !self.props.is_empty() {
            st.serialize_field("props", &self.props)?;
        }
        if !self.on.is_empty() {
            st.serialize_field("on", &self.on)?;
        }
        if !self.slots.is_empty() {
            st.serialize_field("slots", &self.slots)?;
        }
        if !self.children.is_empty() {
            st.serialize_field("children", &self.children)?;
        }
        st.end()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Document {
    pub weft: String,
    /// The `<context>` block (SPEC §2.3), in written order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<Entry>,
    pub root: Node,
}

/// A context entry (SPEC §2.3): a note about the screen, or with `for` about one element. Values
/// stay strings so that validation, not the reader, says what is wrong with them.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Entry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub kind: String,
    pub by: String,
    pub name: String,
    #[serde(rename = "for", skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub text: String,
    /// Markup positions; never serialized and ignored by equality.
    #[serde(skip)]
    pub source: Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Content {
    None,
    Text,
    Nodes,
    Mixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropType {
    String,
    Number,
    Boolean,
    Enum,
    Token,
}

impl PropType {
    pub fn as_str(self) -> &'static str {
        match self {
            PropType::String => "string",
            PropType::Number => "number",
            PropType::Boolean => "boolean",
            PropType::Enum => "enum",
            PropType::Token => "token",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropDefault {
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PropDef {
    pub description: String,
    #[serde(rename = "type")]
    pub kind: PropType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integer: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<PropDefault>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writable: Option<bool>,
    /// A string prop whose literal value is the id of an element of this kind (SPEC §6, `W309`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<String>,
}

impl PropDef {
    pub fn new(description: &str, kind: PropType) -> Self {
        PropDef {
            description: description.to_owned(),
            kind,
            values: None,
            token_type: None,
            min: None,
            max: None,
            integer: None,
            required: None,
            default: None,
            bindable: None,
            writable: None,
            references: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SlotDef {
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_children: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ComponentDef {
    pub description: String,
    pub role: String,
    pub content: Content,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_children: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_parents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_label: Option<bool>,
    /// True for the one kind the document root must be, and that may stand nowhere else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props: Option<Map<PropDef>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<Map<SlotDef>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub states: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
}

impl ComponentDef {
    pub fn prop(&self, name: &str) -> Option<&PropDef> {
        self.props.as_ref()?.get(name)
    }

    pub fn slot(&self, name: &str) -> Option<&SlotDef> {
        self.slots.as_ref()?.get(name)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub weft: String,
    pub name: String,
    pub version: String,
    pub components: Map<ComponentDef>,
    /// A project's fragments (SPEC §10.7). Only the merged catalog of a project has them, so
    /// every tool that is handed that catalog knows them; a catalog file never does.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub fragments: Map<Fragment>,
}

impl Catalog {
    /// The fragment a `<use>` names, when the catalog has it.
    pub fn fragment_of(&self, node: &Node) -> Option<&Fragment> {
        match node.props.get("fragment") {
            Some(Value::String(name)) if node.kind == crate::fragment::USE => {
                self.fragments.get(name)
            }
            _ => None,
        }
    }

    /// What a node answers to: its component, or for a `<use>` its fragment's signature.
    pub fn def_of(&self, node: &Node) -> Option<&ComponentDef> {
        if node.kind == crate::fragment::USE {
            self.fragment_of(node).map(|f| &f.signature)
        } else {
            self.components.get(&node.kind)
        }
    }
}

/// A fragment document with its parameters read once (SPEC §10.7). In JSON it is the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Fragment {
    pub document: Document,
    /// The parameters as the definition a `<use>` answers to: value parameters are its props
    /// (with `fragment` itself), action parameters its events, slot parameters its slots.
    pub signature: ComponentDef,
}

impl Fragment {
    pub fn new(document: Document) -> Self {
        let signature = crate::fragment::signature(&document.root);
        Fragment {
            document,
            signature,
        }
    }
}

impl Serialize for Fragment {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.document.serialize(s)
    }
}

impl<'de> Deserialize<'de> for Fragment {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let json = serde_json::Value::deserialize(d)?;
        // A catalog is untrusted input like a document: only the shape of SPEC §3 is read.
        if crate::validate::json_exceeds_depth(&json)
            || !crate::shape::document_issues(&json).is_empty()
        {
            return Err(serde::de::Error::custom(
                "a fragment is not a document in canonical JSON",
            ));
        }
        Ok(Fragment::new(crate::shape::to_document(&json)))
    }
}
