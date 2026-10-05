//! `weft swiftui` and `weft import-swiftui` (SPEC §9). The catalog and tokens come from the
//! arguments, else the project, else the core catalog and the default tokens; the output
//! directory from `--out-dir`, else the project's `export.swiftui.outDir` or
//! `import.swiftui.outDir`, else standard output.

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use weft_swiftui::{GenerateError, GenerateOptions, ImportOptions, generate, import_swiftui};

use crate::convert::{catalog, emit, finish_import, out_dir, project, strict_document, tokens};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

pub struct ExportArgs {
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

pub fn export(args: ExportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let tokens = tokens(args.tokens.as_deref(), project.as_ref())?;
    let Some(document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
    let errors = &mut std::io::stderr();
    let swift = match generate(
        &document,
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
        },
    ) {
        Ok(swift) => swift,
        Err(GenerateError::Invalid(diagnostics)) => {
            print(&args.file, &diagnostics, errors)?;
            return Ok(DIAGNOSTICS);
        }
        Err(GenerateError::Unsupported(problems)) => {
            for p in problems {
                writeln!(errors, "{}:{p}", args.file.display())?;
            }
            return Ok(DIAGNOSTICS);
        }
    };
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "export",
        "swiftui",
    );
    emit(&swift, &args.file, dir, "swift", out)?;
    Ok(0)
}

pub struct ImportArgs {
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let text = read(&args.file)?;
    let result = import_swiftui(&text, &ImportOptions { catalog: &catalog });
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "import",
        "swiftui",
    );
    finish_import(&args.file, &result, dir, out)
}
