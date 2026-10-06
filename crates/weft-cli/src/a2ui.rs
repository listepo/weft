//! `weft a2ui` and `weft import-a2ui` (SPEC §9). Catalog and output directory resolve as for the
//! other targets: `--catalog`, else the project's; `--out-dir`, else `export.a2ui.outDir` or
//! `import.a2ui.outDir`, else standard output. The messages are one JSON array, which the importer
//! reads back along with a single object or JSON Lines, the form A2UI streams in.

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use weft_interop::a2ui::{from_a2ui, to_a2ui};

use crate::convert::{catalog, emit, finish_import, out_dir, project, strict_document};
use crate::{ProjectArgs, read};

pub struct Args {
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

pub fn export(args: Args, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let catalog = catalog(args.catalog.as_deref(), project)?;
    let Some(document) = strict_document(&args.file, &catalog)? else {
        return Ok(crate::DIAGNOSTICS);
    };
    let exported = to_a2ui(&document);
    crate::convert::report_losses(&args.file, &exported.losses, &mut std::io::stderr())?;
    let mut text = serde_json::to_string_pretty(&exported.messages)?;
    text.push('\n');
    let dir = out_dir(
        args.out_dir,
        project,
        project_dir.as_deref(),
        "export",
        "a2ui",
    );
    // Not `.json` alone: the corpus names these `screen.a2ui.json`.
    emit(&text, &args.file, dir, "a2ui.json", out)?;
    Ok(0)
}

pub fn import(args: Args, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let catalog = catalog(args.catalog.as_deref(), project)?;
    let result = from_a2ui(&read(&args.file)?, &catalog);
    let dir = out_dir(
        args.out_dir,
        project,
        project_dir.as_deref(),
        "import",
        "a2ui",
    );
    finish_import(&args.file, &result, dir, out)
}
