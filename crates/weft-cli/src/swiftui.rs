//! `weft swiftui`, `weft swiftui-tokens` and `weft import-swiftui` (SPEC §9). The catalog and
//! tokens come from the arguments, else the project, else the core catalog and the default
//! tokens; the output directory from `--out-dir`, else the project's `export.swiftui.outDir` or
//! `import.swiftui.outDir`, else standard output.
//! Sample data comes from `--data`, else `export.swiftui.data`, else none. `--context`, else
//! `export.swiftui.context` or `import.swiftui.context`, else `keep`, decides whether the screen's
//! context goes into the source comment and comes back from it.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use weft_catalog::{PROJECT_FILE, appearance};
use weft_swiftui::{
    GenerateError, GenerateOptions, ImportOptions, TOKENS_FILE, generate, generate_tokens,
    import_swiftui,
};

use crate::convert::{
    catalog, emit, finish_import, keeps_context, out_dir, project, sample_data, strict_document,
    switch_or, tokens,
};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

pub struct ExportArgs {
    pub force: bool,

    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--shared-tokens` and `--no-shared-tokens`.
    pub shared_tokens: (bool, bool),
    pub data: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` strips.
    pub context: Option<bool>,
}

pub fn export(args: ExportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let tokens = tokens(args.tokens.as_deref(), project.as_ref())?;
    let data = sample_data(
        args.data,
        project.as_ref(),
        project_dir.as_deref(),
        "swiftui",
    )?;
    let Some(mut document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
    if !keeps_context(args.context, project.as_ref(), "export", "swiftui", true) {
        document.context.clear();
    }
    // A project's screens share one tokens file; a lone screen stays self-contained.
    let (on, off) = args.shared_tokens;
    let shared_tokens = switch_or(
        on,
        off,
        project.as_ref(),
        &["export", "swiftui", "sharedTokens"],
        project.is_some(),
    );
    let errors = &mut std::io::stderr();
    let swift = match generate(
        &document,
        &GenerateOptions {
            catalog: &catalog,
            tokens: &tokens.tokens,
            name: None,
            shared_tokens,
            data: data.as_ref(),
            appearance: appearance(&tokens.modifiers),
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
    emit(&swift, &args.file, dir, "swift", args.force, out)?;
    Ok(0)
}

pub struct TokensArgs {
    pub force: bool,

    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

/// Tokens SwiftUI has no form for are left out with a warning: only a screen that uses one fails.
pub fn export_tokens(args: TokensArgs, out: &mut dyn Write) -> Result<u8> {
    // The project is found as for a document in the working directory.
    let (project, project_dir) = project(Path::new(PROJECT_FILE), args.project)?;
    let tokens = tokens(args.tokens.as_deref(), project.as_ref())?;
    let (swift, skipped) = generate_tokens(&tokens.tokens, appearance(&tokens.modifiers));
    let errors = &mut std::io::stderr();
    for p in skipped {
        writeln!(errors, "weft: token {p}; it is left out")?;
    }
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "export",
        "swiftui",
    );
    emit(
        &swift,
        Path::new(TOKENS_FILE),
        dir,
        "swift",
        args.force,
        out,
    )?;
    Ok(0)
}

pub struct ImportArgs {
    pub force: bool,

    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--context`: `Some(true)` keeps, `Some(false)` drops.
    pub context: Option<bool>,
}

pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let text = read(&args.file)?;
    let mut result = import_swiftui(&text, &ImportOptions { catalog: &catalog });
    if !keeps_context(args.context, project.as_ref(), "import", "swiftui", true) {
        result.document.context.clear();
    }
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "import",
        "swiftui",
    );
    finish_import(&args.file, &result, dir, args.force, out)
}
