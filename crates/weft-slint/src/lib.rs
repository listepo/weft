//! Slint for Weft (SPEC §9, "To Slint" and "From Slint"): `generate` prints one `.slint`
//! component per screen for Slint 1.x and its `std-widgets.slint`, and `import_slint` reads such a
//! file back into the Weft document it came from. The mapping is documented in this crate's README.

mod data;
mod generate;
mod import;
mod names;

pub use generate::{GenerateError, GenerateOptions, generate};
pub use import::{ImportError, ImportOptions, MAX_SOURCE_LENGTH, import_slint};

/// Something a valid document says that the generator cannot express in Slint.
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
