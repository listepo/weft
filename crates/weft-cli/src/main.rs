//! `weft validate`, `weft fmt` and `weft explain`. The catalog comes from `--catalog`; without
//! one, only the syntax layer runs. Exit codes: 0 ok, 1 diagnostics with errors, 2 usage or I/O
//! failure.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use weft_core::{
    Catalog, Diagnostic, Document, Mode, ParseOptions, ValidateOptions, explain, explain_changes,
    has_errors, parse, parse_json, serialize, validate,
};

#[derive(Parser)]
#[command(
    name = "weft",
    version,
    about = "Validate, format and explain Weft documents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a document: markup, or canonical JSON when the file name ends in `.json`.
    Validate {
        file: PathBuf,
        /// Catalog JSON; without it only the syntax layer is checked.
        #[arg(long)]
        catalog: Option<PathBuf>,
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
    /// Read back what each binding, token, event and loop of a markup document means, one per
    /// line, so the meaning can be compared with the instruction behind an edit.
    Explain {
        file: PathBuf,
        /// An older version of the document: print only what was added, removed or changed.
        #[arg(long)]
        against: Option<PathBuf>,
        /// Catalog JSON; without it, boolean and writable props read as plain reads.
        #[arg(long)]
        catalog: Option<PathBuf>,
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
            strict,
        } => {
            let text = read(&file)?;
            let catalog = catalog.as_deref().map(load_catalog).transpose()?;
            if catalog.is_none() {
                eprintln!("weft: no --catalog given; only the syntax layer was checked");
            }
            let mode = if strict { Mode::Strict } else { Mode::Lenient };
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
                    Some(catalog) => validate(
                        &json,
                        &ValidateOptions {
                            catalog: Some(catalog),
                            mode,
                            ..Default::default()
                        },
                    ),
                }
            } else {
                parse(
                    &text,
                    &ParseOptions {
                        catalog: catalog.as_ref(),
                        mode,
                        ..Default::default()
                    },
                )
                .diagnostics
            };
            print(&file, &diagnostics, out)?;
            Ok(if has_errors(&diagnostics) {
                DIAGNOSTICS
            } else {
                0
            })
        }
        Command::Explain {
            file,
            against,
            catalog,
        } => {
            let catalog = catalog.as_deref().map(load_catalog).transpose()?;
            if catalog.is_none() {
                eprintln!(
                    "weft: no --catalog given; boolean and writable props read as plain reads"
                );
            }
            let catalog = catalog.as_ref();
            let Some(document) = load_markup(&file, catalog, out)? else {
                return Ok(DIAGNOSTICS);
            };
            let Some(against) = against else {
                for readback in explain(&document, catalog) {
                    writeln!(out, "{readback}")?;
                }
                return Ok(0);
            };
            let Some(before) = load_markup(&against, catalog, out)? else {
                return Ok(DIAGNOSTICS);
            };
            let changes = explain_changes(&before, &document, catalog);
            if changes.is_empty() {
                eprintln!("weft: no prop, event or loop changed");
            }
            for change in changes {
                writeln!(out, "{change}")?;
            }
            Ok(0)
        }
    }
}

/// A document with errors has no reliable meaning to read back, so its diagnostics are printed as
/// `validate` prints them and the caller stops.
fn load_markup(
    file: &Path,
    catalog: Option<&Catalog>,
    out: &mut dyn Write,
) -> Result<Option<Document>> {
    if file.extension().is_some_and(|e| e == "json") {
        bail!(
            "{}: explain reads markup, not canonical JSON",
            file.display()
        );
    }
    let text = read(file)?;
    let result = parse(
        &text,
        &ParseOptions {
            catalog,
            ..Default::default()
        },
    );
    match result.document {
        Some(document) if !has_errors(&result.diagnostics) => Ok(Some(document)),
        _ => {
            print(file, &result.diagnostics, out)?;
            Ok(None)
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
