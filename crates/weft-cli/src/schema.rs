//! `weft schema` (SPEC §3.1): the JSON Schema of the canonical documents a catalog admits, for a
//! writer whose decoder takes a schema. The project is found from the working directory, as for
//! `weft css-base`; the catalog resolves as for the generators (`--catalog`, else the project's
//! merged catalog, else the core one), and so does the output (`--out-dir`, else
//! `export.schema.outDir`, else standard output).

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use weft_catalog::{DocumentSchemaOptions, PROJECT_FILE, document_schema};

use crate::ProjectArgs;
use crate::convert::{catalog, emit, out_dir, project};

/// `--out-dir` writes `document.schema.json`. A fixed name, not the catalog's: that name comes
/// from the project's catalog file and is no file name anyone checked.
const STEM: &str = "document";

pub struct Args {
    pub catalog: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    pub force: bool,
}

pub fn export(args: Args, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(Path::new(PROJECT_FILE), args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let schema = document_schema(&catalog, &DocumentSchemaOptions::default());
    let mut json = serde_json::to_string_pretty(&schema)?;
    json.push('\n');
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "export",
        "schema",
    );
    emit(&json, Path::new(STEM), dir, "schema.json", args.force, out)?;
    Ok(0)
}
