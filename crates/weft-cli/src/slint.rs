//! `weft slint` and `weft import-slint` (SPEC §9, §10.6). The catalog and tokens come from the
//! arguments, else the project, else the core catalog and the default tokens; the output
//! directory from `--out-dir`, else the project's `export.slint.outDir` or `import.slint.outDir`,
//! else standard output. `--name` is the component name the generator is given.

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use weft_core::serialize;
use weft_slint::{
    GenerateError, GenerateOptions, ImportError, ImportOptions, generate, import_slint,
};

use crate::convert::{catalog, emit, keeps_context, out_dir, project, strict_document, tokens};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

pub struct ExportArgs {
    pub file: PathBuf,
    pub catalog: Vec<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub name: Option<String>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` strips.
    pub context: Option<bool>,
    pub force: bool,
}

pub fn export(args: ExportArgs, out: &mut dyn Write) -> Result<u8> {
    let (loaded, project_dir) = project(&args.file, args.project)?;
    let loaded = loaded.as_ref();
    let catalog = catalog(&args.catalog, loaded)?;
    let tokens = tokens(args.tokens.as_deref(), loaded)?;
    let Some(mut document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
    if !keeps_context(args.context, loaded, "export", "slint", true) {
        document.context.clear();
    }
    let slint = match generate(
        &document,
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens.tokens,
            name: args.name.as_deref(),
        },
    ) {
        Ok(slint) => slint,
        Err(GenerateError::Invalid(diagnostics)) => {
            print(&args.file, &diagnostics, &mut std::io::stderr())?;
            return Ok(DIAGNOSTICS);
        }
        Err(GenerateError::Unsupported(problems)) => {
            let errors = &mut std::io::stderr();
            for problem in problems {
                writeln!(errors, "{}:{problem}", args.file.display())?;
            }
            return Ok(DIAGNOSTICS);
        }
    };
    let dir = out_dir(
        args.out_dir,
        loaded,
        project_dir.as_deref(),
        "export",
        "slint",
    );
    emit(&slint, &args.file, dir, "slint", args.force, out)?;
    Ok(0)
}

pub struct ImportArgs {
    pub file: PathBuf,
    pub catalog: Vec<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` drops.
    pub context: Option<bool>,
    pub force: bool,
}

pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (loaded, project_dir) = project(&args.file, args.project)?;
    let loaded = loaded.as_ref();
    let catalog = catalog(&args.catalog, loaded)?;
    // Regeneration is how the importer decides the file is still the one it printed, so the
    // tokens have to be the ones generation used. There is no `--tokens` on import: the project,
    // or the default tokens, are that set.
    let tokens = tokens(None, loaded)?;
    let text = read(&args.file)?;
    let mut document = match import_slint(
        &text,
        &ImportOptions {
            catalog: &catalog,
            tokens: &tokens.tokens,
        },
    ) {
        Ok(document) => document,
        Err(ImportError::Invalid(diagnostics)) => {
            print(&args.file, &diagnostics, &mut std::io::stderr())?;
            return Ok(DIAGNOSTICS);
        }
        Err(error) => {
            writeln!(std::io::stderr(), "{}: {error}", args.file.display())?;
            return Ok(DIAGNOSTICS);
        }
    };
    let dir = out_dir(
        args.out_dir,
        loaded,
        project_dir.as_deref(),
        "import",
        "slint",
    );
    if !keeps_context(args.context, loaded, "import", "slint", true) {
        document.context.clear();
    }
    emit(
        &serialize(&document),
        &args.file,
        dir,
        "weft",
        args.force,
        out,
    )?;
    Ok(0)
}
