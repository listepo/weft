//! `weft validate` and `weft fmt`. The catalog comes from `--catalog`; without one, only the
//! syntax layer runs. Exit codes: 0 ok, 1 diagnostics with errors, 2 usage or I/O failure.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use weft_core::{
    Catalog, Diagnostic, Mode, ParseOptions, ValidateOptions, has_errors, parse, parse_json,
    serialize, validate,
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
