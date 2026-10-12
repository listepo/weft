//! The document core of WeftStudio, the desktop editor (T69): one Weft screen opened from a
//! `.weft` file with the project above it (SPEC §10.1), saved canonical, and the input a Slint
//! surface renders. No Slint crate here: the shells' FFI crate compiles the source with the
//! `weft-slint` runtime, so this core stays testable on its own.

use std::io::Read;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value as Json;
use weft_catalog::{
    CatalogError, DEFAULT_TOKENS_JSON, Project, ProjectFileError, Token, WriteError, core_catalog,
    find_project, load_project_file, load_tokens, read_project_member, token_types, write_atomic,
};
use weft_core::{Catalog, Diagnostic, Document, Mode, ParseOptions, parse, parse_json, serialize};
use weft_slint::{GenerateError, GenerateOptions, MAX_SOURCE_LENGTH, generate};

/// The largest file `open` reads: the screen's markup travels inside the generated Slint source,
/// so the bound `weft-slint` puts on Slint input is the bound here.
pub const MAX_FILE_BYTES: usize = MAX_SOURCE_LENGTH;

#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("cannot read {}", .0.display())]
    Read(PathBuf, #[source] std::io::Error),
    #[error("{} is larger than {MAX_FILE_BYTES} bytes", .0.display())]
    TooLarge(PathBuf),
    /// The markup does not parse; a document with only validation errors still opens.
    #[error("{} has syntax errors", .path.display())]
    Syntax {
        path: PathBuf,
        diagnostics: Vec<Diagnostic>,
    },
    #[error(transparent)]
    Write(#[from] WriteError),
    /// The generator refused the screen: it does not validate strictly, or Slint cannot say it.
    #[error(transparent)]
    Generate(#[from] GenerateError),
    /// `render.data` names a file that is missing, outside the project, or not JSON.
    #[error("the sample data {0} cannot be read as JSON inside the project")]
    SampleData(String),
    #[error(transparent)]
    Project(#[from] ProjectFileError),
    #[error("the embedded core catalog or default tokens are broken")]
    Embedded(#[from] CatalogError),
}

/// What a shell shows: the window title, the screen and the problems. `dirty` stays false until
/// editing arrives (T69 milestone 3).
#[derive(Debug, Serialize)]
pub struct State {
    pub path: PathBuf,
    pub title: String,
    pub screen: Option<String>,
    pub dirty: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub project: Option<PathBuf>,
    pub project_diagnostics: Vec<Diagnostic>,
}

/// The generated Slint source and the sample data its properties are filled from.
#[derive(Debug)]
pub struct RenderInput {
    pub source: String,
    pub data: Option<Json>,
}

struct OpenProject {
    file: PathBuf,
    project: Project,
    diagnostics: Vec<Diagnostic>,
}

pub struct Editor {
    path: PathBuf,
    document: Document,
    diagnostics: Vec<Diagnostic>,
    project: Option<OpenProject>,
    catalog: Catalog,
    tokens: IndexMap<String, Token>,
}

impl Editor {
    /// Opens the markup at `path` with the project found above it. Project problems never stop
    /// the open (SPEC §10.2); they are kept for the state, as the document's own diagnostics are.
    pub fn open(path: &Path) -> Result<Editor, EditorError> {
        let text = read_capped(path)?;
        let project = match find_project(path) {
            Some(file) => {
                let loaded = load_project_file(&file, Mode::Lenient)?;
                Some(OpenProject {
                    file,
                    project: loaded.project,
                    diagnostics: loaded.diagnostics,
                })
            }
            None => None,
        };
        let catalog = match &project {
            Some(open) => open.project.catalog.clone(),
            None => core_catalog()?,
        };
        let tokens = match project
            .as_ref()
            .and_then(|open| open.project.tokens.clone())
        {
            Some(tokens) => tokens,
            None => {
                let json = serde_json::from_str(DEFAULT_TOKENS_JSON).map_err(CatalogError::from)?;
                load_tokens(&json).tokens
            }
        };
        let types = token_types(&tokens);
        let parsed = parse(
            &text,
            &ParseOptions {
                catalog: Some(&catalog),
                mode: Mode::Lenient,
                tokens: Some(&types),
                actions: project
                    .as_ref()
                    .and_then(|open| open.project.actions.as_deref()),
            },
        );
        let Some(document) = parsed.document else {
            return Err(EditorError::Syntax {
                path: path.to_owned(),
                diagnostics: parsed.diagnostics,
            });
        };
        Ok(Editor {
            path: path.to_owned(),
            document,
            diagnostics: parsed.diagnostics,
            project,
            catalog,
            tokens,
        })
    }

    /// Writes the canonical markup atomically to `path`, or to the current path without one;
    /// a save elsewhere makes that the document's path. A failed save changes nothing.
    pub fn save(&mut self, path: Option<&Path>) -> Result<(), EditorError> {
        let target = path.unwrap_or(&self.path).to_owned();
        write_atomic(&target, &serialize(&self.document))?;
        self.path = target;
        Ok(())
    }

    pub fn state(&self) -> State {
        State {
            title: self
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: self.path.clone(),
            screen: self.document.root.id.clone(),
            dirty: false,
            diagnostics: self.diagnostics.clone(),
            project: self.project.as_ref().map(|open| open.file.clone()),
            project_diagnostics: self
                .project
                .as_ref()
                .map(|open| open.diagnostics.clone())
                .unwrap_or_default(),
        }
    }

    /// The screen as Slint, and the project's `render.data` sample data (SPEC §10.6) when set.
    pub fn render_input(&self) -> Result<RenderInput, EditorError> {
        let source = generate(
            &self.document,
            &GenerateOptions {
                catalog: &self.catalog,
                tokens: &self.tokens,
                name: None,
            },
        )?;
        Ok(RenderInput {
            source,
            data: self.sample_data()?,
        })
    }

    fn sample_data(&self) -> Result<Option<Json>, EditorError> {
        let Some(open) = &self.project else {
            return Ok(None);
        };
        let Some(name) = open
            .project
            .setting(&["render", "data"])
            .and_then(Json::as_str)
        else {
            return Ok(None);
        };
        // The same containment as the project's own members: the data is untrusted input.
        let dir = open.file.parent().unwrap_or(Path::new("."));
        read_project_member(dir, name)
            .and_then(|text| parse_json(&text).ok())
            .map(Some)
            .ok_or_else(|| EditorError::SampleData(name.to_owned()))
    }
}

/// Reads at most one byte past the limit, so a huge file is refused without loading it.
fn read_capped(path: &Path) -> Result<String, EditorError> {
    let read_error = |e| EditorError::Read(path.to_owned(), e);
    let file = std::fs::File::open(path).map_err(read_error)?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(EditorError::TooLarge(path.to_owned()));
    }
    String::from_utf8(bytes)
        .map_err(|e| read_error(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}
