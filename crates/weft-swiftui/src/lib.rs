//! SwiftUI for Weft: `generate` prints one self-contained Swift file per screen (iOS 17 and
//! macOS 14 or later), and `import_swiftui` (feature `import`) reads Swift source back into a
//! Weft document with a loss table. The mapping is documented in this crate's README; code that
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

/// The default design tokens of `packages/catalog`, for callers that have no token file.
pub const DEFAULT_TOKENS_JSON: &str =
    include_str!("../../../packages/catalog/tokens/default.tokens.json");

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
    pub const EACH: &str = "each";
}
