//! SwiftUI for Weft: `generate` prints one Swift file per screen (iOS 17 and macOS 14 or later),
//! self-contained or reading the project's shared tokens that `generate_tokens` prints, and
//! `import_swiftui` (feature `import`) reads Swift source back into a Weft document with a loss
//! table. The mapping is documented in this crate's README; code that
//! `generate` printed imports back to the same document.

mod data;
mod generate;
#[cfg(feature = "import")]
mod import;
mod swift;
mod theme;

pub use generate::{GenerateError, GenerateOptions, generate};
#[cfg(feature = "import")]
pub use import::{
    ImportOptions, ImportResult, Loss, LossKind, MAX_DEPTH, MAX_NODES, MAX_SOURCE_LENGTH,
    import_swiftui,
};
pub use theme::{TOKENS_FILE, TOKENS_TYPE, generate_tokens};

/// The default design tokens of `packages/catalog`, for callers that have no token file.
pub use weft_catalog::DEFAULT_TOKENS_JSON;

/// Something a valid document says that the generator cannot express in Swift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The element path (SPEC §6.1) or the binding path the problem is about.
    pub path: String,
    pub message: String,
}

impl Unsupported {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Unsupported {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

mod kinds {
    use std::sync::OnceLock;

    pub const EACH: &str = "each";

    /// The kinds of the core catalog: any other kind is the project's own, and a screen calls the
    /// view the app writes for it.
    pub fn is_core_kind(kind: &str) -> bool {
        static KINDS: OnceLock<Vec<String>> = OnceLock::new();
        KINDS
            .get_or_init(|| {
                weft_catalog::core_catalog()
                    .map(|c| c.components.keys().cloned().collect())
                    .unwrap_or_default()
            })
            .iter()
            .any(|k| k == kind)
    }
}
