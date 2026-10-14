//! WinUI 3 for Weft (SPEC §9, "To WinUI" and "From WinUI"): `generate` writes one XAML `Page`
//! per screen for the Windows App SDK, bound with `{x:Bind}` to a generated view model, and
//! `import_winui` reads such a page back into the Weft document its source comment holds. The
//! design is `docs/winui-design.md`. Pure Rust: nothing here needs Windows.

mod data;
mod generate;
mod import;
mod xaml;

pub use generate::{GenerateError, GenerateOptions, Generated, generate};
pub use import::{ImportError, ImportOptions, MAX_SOURCE_LENGTH, import_winui};

/// Something a valid document says that the generator cannot express in WinUI.
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
