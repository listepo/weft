use serde::{Deserialize, Serialize};
use weft_core::Map;

use crate::loss::LossKind;
use crate::props::Scalar;

/// A loss the source already knows about for one node; reported with the node's path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub kind: LossKind,
    pub note: String,
}

/// The neutral tree importers produce before the catalog is consulted: one entry per
/// accessibility node, or per text run (role `text`, the text in `name`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sem {
    pub role: String,
    pub name: String,
    /// ARIA states as a snapshot reports them: checked, disabled, selected, level, invalid, busy, …
    #[serde(default)]
    pub states: Map<Scalar>,
    /// Values only a source carries, keyed by the catalog prop they feed (type, value, href, …).
    #[serde(default)]
    pub props: Map<String>,
    /// A kind the source names directly (Weft renderer markup for role-less kinds such as `text`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A Weft id the source carries, already checked to be valid and unique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The source's own element id and `aria-labelledby` references, which pair tab panels with
    /// their tabs.
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labelled_by: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
    #[serde(default)]
    pub children: Vec<Sem>,
}

impl Sem {
    pub fn new(role: impl Into<String>, name: impl Into<String>) -> Self {
        Sem {
            role: role.into(),
            name: name.into(),
            ..Sem::default()
        }
    }

    /// A text run.
    pub fn text(name: impl Into<String>) -> Self {
        Sem::new("text", name)
    }
}
