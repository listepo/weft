//! `weft validate` and `weft fmt`. The catalog, tokens, actions and data schema come from the
//! project file (`weft.json`, SPEC §10) found above the document or given with `--project`;
//! `--catalog` replaces the project's catalog. Without either, only the syntax layer runs.
//! Exit codes: 0 ok, 1 diagnostics with errors, 2 usage or I/O failure.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use weft_catalog::{PROJECT_FILE, Project, ProjectOptions, load_project_text, token_types};
use weft_core::{
    Catalog, DataCheckOptions, Diagnostic, Mode, ParseOptions, ValidateOptions, check_data,
    check_data_json, has_errors, parse, parse_json, serialize, validate,
};

#[derive(Parser)]
#[command(name = "weft", version, about = "Validate and format Weft documents")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a document: markup, or canonical JSON when the file name ends in `.json`.
    Validate {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog.
        #[arg(long)]
        catalog: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` above the document is used.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Report unknown content as errors instead of warnings.
        #[arg(long)]
        strict: bool,
    },
    /// Print the canonical markup of a document.
    Fmt {
        file: PathBuf,
        /// Rewrite the file instead of printing; untouched when already canonical.
        #[arg(long)]
        write: bool,
    },
}

const USAGE_ERROR: u8 = 2;
const DIAGNOSTICS: u8 = 1;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(if e.use_stderr() { USAGE_ERROR } else { 0 });
        }
    };
    match run(cli.command, &mut std::io::stdout().lock()) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("weft: {e:#}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

fn read(file: &Path) -> Result<String> {
    std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))
}

/// SPEC §10.1: the first `weft.json` in the document's directory or one above it.
fn find_project(document: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(document).ok()?;
    absolute
        .ancestors()
        .skip(1)
        .map(|dir| dir.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

/// The project and its diagnostics. Files it names are read relative to its directory; the
/// loader has already refused names that leave it.
fn load_project(file: &Path, mode: Mode) -> Result<(Project, Vec<Diagnostic>)> {
    let text = read(file)?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let read_member = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
    let loaded = load_project_text(
        &text,
        &ProjectOptions {
            read: Some(&read_member),
            mode,
            ..Default::default()
        },
    )
    .context("the embedded core catalog is broken")?;
    Ok((loaded.project, loaded.diagnostics))
}

fn load_catalog(file: &Path) -> Result<Catalog> {
    let text = read(file)?;
    let json =
        parse_json(&text).with_context(|| format!("catalog {} is not JSON", file.display()))?;
    serde_json::from_value(json)
        .with_context(|| format!("catalog {} is not a Weft catalog", file.display()))
}

/// I/O and usage failures are errors; documents with diagnostics are an exit code.
fn run(command: Command, out: &mut dyn Write) -> Result<u8> {
    match command {
        Command::Fmt { file, write } => {
            let text = read(&file)?;
            let result = parse(&text, &ParseOptions::default());
            let Some(document) = result.document else {
                print(&file, &result.diagnostics, out)?;
                return Ok(DIAGNOSTICS);
            };
            let formatted = serialize(&document);
            if !write {
                out.write_all(formatted.as_bytes())?;
            } else if formatted != text {
                std::fs::write(&file, formatted)
                    .with_context(|| format!("cannot write {}", file.display()))?;
            }
            Ok(0)
        }
        Command::Validate {
            file,
            catalog,
            project,
            no_project,
            strict,
        } => {
            let text = read(&file)?;
            let mode = if strict { Mode::Strict } else { Mode::Lenient };
            let project_file = if no_project {
                None
            } else {
                project.or_else(|| find_project(&file))
            };
            let (project, project_diagnostics) = match &project_file {
                Some(path) => {
                    let (project, diagnostics) = load_project(path, mode)?;
                    print(path, &diagnostics, out)?;
                    (Some(project), diagnostics)
                }
                None => (None, Vec::new()),
            };
            let explicit = catalog.as_deref().map(load_catalog).transpose()?;
            let catalog = explicit.or_else(|| project.as_ref().map(|p| p.catalog.clone()));
            if catalog.is_none() {
                eprintln!("weft: no --catalog and no project; only the syntax layer was checked");
            }
            let tokens = project
                .as_ref()
                .and_then(|p| p.tokens.as_ref())
                .map(token_types);
            let actions = project.as_ref().and_then(|p| p.actions.as_deref());
            let data = project.as_ref().and_then(|p| p.data.as_ref());
            let diagnostics = if file.extension().is_some_and(|e| e == "json") {
                let json = match parse_json(&text) {
                    Ok(json) => json,
                    Err(e) => {
                        eprintln!("{}: {e}", file.display());
                        return Ok(DIAGNOSTICS);
                    }
                };
                match &catalog {
                    None => vec![],
                    Some(catalog) => {
                        let mut found = validate(
                            &json,
                            &ValidateOptions {
                                catalog: Some(catalog),
                                mode,
                                tokens: tokens.as_ref(),
                                actions,
                            },
                        );
                        if let Some(data) = data {
                            found.extend(check_data_json(
                                &json,
                                &DataCheckOptions { catalog, data },
                            ));
                        }
                        found
                    }
                }
            } else {
                let result = parse(
                    &text,
                    &ParseOptions {
                        catalog: catalog.as_ref(),
                        mode,
                        tokens: tokens.as_ref(),
                        actions,
                    },
                );
                let mut found = result.diagnostics;
                if let (Some(document), Some(catalog), Some(data)) =
                    (&result.document, &catalog, data)
                {
                    found.extend(check_data(document, &DataCheckOptions { catalog, data }));
                }
                found
            };
            print(&file, &diagnostics, out)?;
            Ok(
                if has_errors(&diagnostics) || has_errors(&project_diagnostics) {
                    DIAGNOSTICS
                } else {
                    0
                },
            )
        }
    }
}

/// One line per diagnostic: `file:line:col code message — hint`; JSON input has a path instead.
fn print(file: &Path, diagnostics: &[Diagnostic], out: &mut dyn Write) -> Result<()> {
    let file = file.display();
    for d in diagnostics {
        let place = match d.line {
            Some(line) => format!("{file}:{line}:{}", d.column.unwrap_or(1)),
            None => format!("{file}:{}", d.path),
        };
        let hint = d
            .hint
            .as_ref()
            .map(|h| format!(" — {h}"))
            .unwrap_or_default();
        writeln!(out, "{place} {} {}{hint}", d.code.as_str(), d.message)?;
    }
    Ok(())
}
