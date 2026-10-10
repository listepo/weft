//! The loss table: what an import could not carry over, one entry per place (SPEC §9).

use serde::{Deserialize, Serialize};
use weft_core::{Diagnostic, Document, Node, WEFT_VERSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LossKind {
    Ids,
    Bindings,
    Actions,
    Tokens,
    Layout,
    Repetition,
    Slots,
    Hidden,
    Props,
    Values,
    Names,
    Kinds,
    Text,
    Structure,
    /// Context entries about layers a designer removed (design tools).
    Context,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loss {
    pub kind: LossKind,
    pub path: String,
    pub note: String,
}

/// The losses of one import, in the order they were found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Losses(pub Vec<Loss>);

impl Losses {
    pub fn push(&mut self, kind: LossKind, path: impl Into<String>, note: impl Into<String>) {
        self.0.push(Loss {
            kind,
            path: path.into(),
            note: note.into(),
        });
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportResult {
    pub document: Document,
    pub losses: Vec<Loss>,
    pub diagnostics: Vec<Diagnostic>,
}

/// The result when nothing could be read: the smallest document SPEC §2 allows.
pub fn empty_result(diagnostics: Vec<Diagnostic>) -> ImportResult {
    let mut root = Node::new("screen");
    root.id = Some("screen".into());
    ImportResult {
        document: Document {
            weft: WEFT_VERSION.into(),
            context: Vec::new(),
            root,
        },
        losses: vec![Loss {
            kind: LossKind::Structure,
            path: "/screen#screen".into(),
            note: "nothing could be imported".into(),
        }],
        diagnostics,
    }
}
