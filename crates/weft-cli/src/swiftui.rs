//! `weft swiftui`, `weft swiftui-tokens` and `weft import-swiftui` (SPEC §9). The catalog and
//! tokens come from the arguments, else the project, else the core catalog and the default
//! tokens; the output directory from `--out-dir`, else the project's `export.swiftui.outDir` or
//! `import.swiftui.outDir`, else standard output.
//! Sample data comes from `--data`, else `export.swiftui.data`, else none.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use weft_catalog::PROJECT_FILE;
use weft_swiftui::{
    GenerateError, GenerateOptions, ImportOptions, TOKENS_FILE, generate, generate_tokens,
    import_swiftui,
};

use crate::convert::{
    catalog, emit, finish_import, out_dir, project, sample_data, strict_document, switch_or, tokens,
};
use crate::{DIAGNOSTICS, ProjectArgs, print, read};

pub struct ExportArgs {
    pub file: PathBuf,
    pub catalog: Option<PathBuf>,
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    /// `--shared-tokens` and `--no-shared-tokens`.
    pub shared_tokens: (bool, bool),
    pub data: Option<PathBuf>,
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
    let Some(document) = strict_document(&args.file, &catalog)? else {
        return Ok(DIAGNOSTICS);
    };
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
            tokens: &tokens,
            name: None,
            shared_tokens,
            data: data.as_ref(),
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

pub struct TokensArgs {
    pub tokens: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
}

/// Tokens SwiftUI has no form for are left out with a warning: only a screen that uses one fails.
pub fn export_tokens(args: TokensArgs, out: &mut dyn Write) -> Result<u8> {
    // The project is found as for a document in the working directory.
    let (project, project_dir) = project(Path::new(PROJECT_FILE), args.project)?;
    let tokens = tokens(args.tokens.as_deref(), project.as_ref())?;
    let (swift, skipped) = generate_tokens(&tokens);
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
    emit(&swift, Path::new(TOKENS_FILE), dir, "swift", out)?;
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
