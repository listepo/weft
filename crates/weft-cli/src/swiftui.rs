//! `weft swiftui` and `weft import-swiftui` (SPEC §9). The catalog and tokens come from the
//! arguments, else the project, else the core catalog and the default tokens; the output
//! directory from `--out-dir`, else the project's `export.swiftui.outDir` or
//! `import.swiftui.outDir`, else standard output.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use weft_catalog::{Project, Token, core_catalog, load_tokens};
use weft_core::{Catalog, Mode, ParseOptions, has_errors, parse, parse_json, serialize};
use weft_swiftui::{GenerateError, GenerateOptions, ImportOptions, generate, import_swiftui};

use crate::{DIAGNOSTICS, ProjectArgs, load_catalog, load_project, print, project_file, read};

/// The project for `file`, with the directory its file names are relative to. Its diagnostics go
/// to stderr: stdout carries the output.
fn project(file: &Path, args: ProjectArgs) -> Result<(Option<Project>, Option<PathBuf>)> {
    let Some(project_path) = project_file(file, args) else {
        return Ok((None, None));
    };
    let (project, diagnostics) = load_project(&project_path, Mode::Lenient)?;
    print(&project_path, &diagnostics, &mut std::io::stderr())?;
    let dir = project_path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok((Some(project), Some(dir)))
}

fn catalog(explicit: Option<&Path>, project: Option<&Project>) -> Result<Catalog> {
    match (explicit, project) {
        (Some(file), _) => load_catalog(file),
        (None, Some(project)) => Ok(project.catalog.clone()),
        (None, None) => core_catalog().context("the embedded core catalog is broken"),
    }
}

fn tokens(explicit: Option<&Path>, project: Option<&Project>) -> Result<IndexMap<String, Token>> {
    if let Some(file) = explicit {
        let json = parse_json(&read(file)?)
            .with_context(|| format!("tokens {} are not JSON", file.display()))?;
        let loaded = load_tokens(&json);
        for p in &loaded.problems {
            eprintln!("{}:{} {:?} {}", file.display(), p.path, p.code, p.message);
        }
        return Ok(loaded.tokens);
    }
    if let Some(tokens) = project.and_then(|p| p.tokens.clone()) {
        return Ok(tokens);
    }
    let json = parse_json(weft_swiftui::DEFAULT_TOKENS_JSON)
        .context("the embedded default tokens are broken")?;
    Ok(load_tokens(&json).tokens)
}

/// `--out-dir` (relative to the working directory), else the project's setting (relative to the
/// project file), else none: print.
fn out_dir(
    flag: Option<PathBuf>,
    project: Option<&Project>,
    project_dir: Option<&Path>,
    section: &str,
) -> Option<PathBuf> {
    flag.or_else(|| {
        let name = project?
            .setting(&[section, "swiftui", "outDir"])?
            .as_str()?;
        Some(project_dir?.join(name))
    })
}

/// Prints `text`, or writes it to `<dir>/<stem of file>.<extension>`.
fn emit(
    text: &str,
    file: &Path,
    dir: Option<PathBuf>,
    extension: &str,
    out: &mut dyn Write,
) -> Result<()> {
    let Some(dir) = dir else {
        out.write_all(text.as_bytes())?;
        return Ok(());
    };
    let stem = file
        .file_stem()
        .with_context(|| format!("{} has no file name", file.display()))?;
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    // Not `with_extension`: it would cut a stem with a dot in it (`login.v2`).
    let mut name = stem.to_os_string();
    name.push(".");
    name.push(extension);
    let target = dir.join(name);
    std::fs::write(&target, text).with_context(|| format!("cannot write {}", target.display()))
}

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
    let text = read(&args.file)?;
    let result = parse(
        &text,
        &ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            ..Default::default()
        },
    );
    let errors = &mut std::io::stderr();
    let document = match result.document {
        Some(document) if !has_errors(&result.diagnostics) => document,
        _ => {
            print(&args.file, &result.diagnostics, errors)?;
            return Ok(DIAGNOSTICS);
        }
    };
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

/// Losses are not failures: they go to stderr, one per line, and the exit code is 0 unless the
/// importer reports an error.
pub fn import(args: ImportArgs, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let catalog = catalog(args.catalog.as_deref(), project.as_ref())?;
    let text = read(&args.file)?;
    let result = import_swiftui(&text, &ImportOptions { catalog: &catalog });
    let errors = &mut std::io::stderr();
    print(&args.file, &result.diagnostics, errors)?;
    for loss in &result.losses {
        let kind = serde_json::to_value(loss.kind)?;
        let kind = kind.as_str().unwrap_or_default();
        writeln!(
            errors,
            "{}:{} loss {kind}: {}",
            args.file.display(),
            loss.path,
            loss.note
        )?;
    }
    let dir = out_dir(
        args.out_dir,
        project.as_ref(),
        project_dir.as_deref(),
        "import",
    );
    emit(&serialize(&result.document), &args.file, dir, "weft", out)?;
    Ok(if has_errors(&result.diagnostics) {
        DIAGNOSTICS
    } else {
        0
    })
}
