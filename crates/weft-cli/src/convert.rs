//! What the generators and importers share (SPEC §9, §10.6): the project of the input, its
//! catalog and tokens unless arguments replace them, the strict check before generating, where
//! output goes, and how losses are reported. Settings of the project stand in for missing flags.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use weft_catalog::{
    DEFAULT_TOKENS_JSON, Project, ProjectOptions, Token, TokenModifier, core_catalog, is_resolver,
    load_project, load_tokens,
};
use weft_core::{Catalog, Document, Mode, ParseOptions, has_errors, parse, parse_json, serialize};
use weft_import::ImportResult;

use crate::load_project as load_project_file;
use crate::{DIAGNOSTICS, ProjectArgs, load_catalog, print, project_file, read};

/// The project for `file`, with the directory its file names are relative to. Its diagnostics go
/// to stderr: stdout carries the output.
pub fn project(file: &Path, args: ProjectArgs) -> Result<(Option<Project>, Option<PathBuf>)> {
    let Some(project_path) = project_file(file, args) else {
        return Ok((None, None));
    };
    let (project, diagnostics) = load_project_file(&project_path, Mode::Lenient)?;
    print(&project_path, &diagnostics, &mut std::io::stderr())?;
    let dir = project_path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    Ok((Some(project), Some(dir)))
}

pub fn catalog(explicit: Option<&Path>, project: Option<&Project>) -> Result<Catalog> {
    match (explicit, project) {
        (Some(file), _) => load_catalog(file),
        (None, Some(project)) => Ok(project.catalog.clone()),
        (None, None) => core_catalog().context("the embedded core catalog is broken"),
    }
}

/// The tokens a generator works with: the default context, and every context of each modifier
/// when they come from a resolver (SPEC §10.3).
pub struct TokenSet {
    pub tokens: IndexMap<String, Token>,
    pub modifiers: Vec<TokenModifier>,
}

/// `--tokens` (a token file or a resolver), else the project's, else the default tokens.
pub fn tokens(explicit: Option<&Path>, project: Option<&Project>) -> Result<TokenSet> {
    if let Some(file) = explicit {
        let json = parse_json(&read(file)?)
            .with_context(|| format!("tokens {} are not JSON", file.display()))?;
        if is_resolver(&json) {
            return resolver(file);
        }
        let loaded = load_tokens(&json);
        for p in &loaded.problems {
            eprintln!("{}:{} {:?} {}", file.display(), p.path, p.code, p.message);
        }
        return Ok(TokenSet {
            tokens: loaded.tokens,
            modifiers: vec![],
        });
    }
    if let Some(project) = project
        && let Some(tokens) = project.tokens.clone()
    {
        return Ok(TokenSet {
            tokens,
            modifiers: project.modifiers.clone(),
        });
    }
    let json = parse_json(DEFAULT_TOKENS_JSON).context("the embedded default tokens are broken")?;
    Ok(TokenSet {
        tokens: load_tokens(&json).tokens,
        modifiers: vec![],
    })
}

/// A resolver given as `--tokens` loads as the `tokens` of a project beside it, so its references
/// follow the same rules (SPEC §10.3); its diagnostics point into the resolver file itself.
fn resolver(file: &Path) -> Result<TokenSet> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("{} has no file name", file.display()))?;
    let read_member = |member: &str| std::fs::read_to_string(dir.join(member)).ok();
    let loaded = load_project(
        &serde_json::json!({ "tokens": name }),
        &ProjectOptions {
            read: Some(&read_member),
            ..Default::default()
        },
    )
    .context("the embedded core catalog is broken")?;
    let diagnostics: Vec<_> = loaded
        .diagnostics
        .into_iter()
        .map(|mut d| {
            d.path = d.path.replacen("#/tokens", "#", 1);
            d
        })
        .collect();
    print(file, &diagnostics, &mut std::io::stderr())?;
    Ok(TokenSet {
        tokens: loaded.project.tokens.unwrap_or_default(),
        modifiers: loaded.project.modifiers,
    })
}

/// Sample data for a generator: `--data` (relative to the working directory), else the project's
/// `export.<target>.data` (relative to the project file), else none.
pub fn sample_data(
    flag: Option<PathBuf>,
    project: Option<&Project>,
    project_dir: Option<&Path>,
    target: &str,
) -> Result<Option<serde_json::Value>> {
    let file = flag.or_else(|| {
        let name = project?.setting(&["export", target, "data"])?.as_str()?;
        Some(project_dir?.join(name))
    });
    let Some(file) = file else {
        return Ok(None);
    };
    let json = parse_json(&read(&file)?)
        .with_context(|| format!("sample data {} is not JSON", file.display()))?;
    Ok(Some(json))
}

/// A boolean flag pair (`--x` / `--no-x`), else the project's setting at `path`, else false.
pub fn switch(on: bool, off: bool, project: Option<&Project>, path: &[&str]) -> bool {
    switch_or(on, off, project, path, false)
}

/// [`switch`] with its own default.
pub fn switch_or(
    on: bool,
    off: bool,
    project: Option<&Project>,
    path: &[&str],
    default: bool,
) -> bool {
    if on || off {
        return on;
    }
    project
        .and_then(|p| p.setting(path))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(default)
}

/// `--out-dir` (relative to the working directory), else the project's `<section>.<target>.outDir`
/// (relative to the project file), else none: print.
pub fn out_dir(
    flag: Option<PathBuf>,
    project: Option<&Project>,
    project_dir: Option<&Path>,
    section: &str,
    target: &str,
) -> Option<PathBuf> {
    flag.or_else(|| {
        let name = project?.setting(&[section, target, "outDir"])?.as_str()?;
        Some(project_dir?.join(name))
    })
}

/// Prints `text`, or writes it to `<dir>/<stem of file>.<extension>`.
pub fn emit(
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

/// The screen to generate from, checked strictly against `catalog`: generators write code only
/// for a valid screen. `None` after printing its diagnostics.
pub fn strict_document(file: &Path, catalog: &Catalog) -> Result<Option<Document>> {
    let text = read(file)?;
    let result = parse(
        &text,
        &ParseOptions {
            catalog: Some(catalog),
            mode: Mode::Strict,
            ..Default::default()
        },
    );
    match result.document {
        Some(document) if !has_errors(&result.diagnostics) => Ok(Some(document)),
        _ => {
            print(file, &result.diagnostics, &mut std::io::stderr())?;
            Ok(None)
        }
    }
}

/// Losses are not failures: they go to stderr, one per line, and the exit code is 0 unless the
/// importer reports an error. The screen goes to stdout or `dir`.
pub fn finish_import(
    file: &Path,
    result: &ImportResult,
    dir: Option<PathBuf>,
    out: &mut dyn Write,
) -> Result<u8> {
    let errors = &mut std::io::stderr();
    print(file, &result.diagnostics, errors)?;
    for loss in &result.losses {
        let kind = serde_json::to_value(loss.kind)?;
        let kind = kind.as_str().unwrap_or_default();
        writeln!(
            errors,
            "{}:{} loss {kind}: {}",
            file.display(),
            loss.path,
            loss.note
        )?;
    }
    emit(&serialize(&result.document), file, dir, "weft", out)?;
    Ok(if has_errors(&result.diagnostics) {
        DIAGNOSTICS
    } else {
        0
    })
}
