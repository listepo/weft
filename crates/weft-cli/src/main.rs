//! `weft validate`, `weft fmt`, `weft explain`, and the generators and importers of SPEC §9:
//! SwiftUI (`weft swiftui`, `weft import-swiftui`, in `swiftui.rs`), Slint (`weft slint`,
//! `weft import-slint`, in `slint.rs`) and the web targets (`weft
//! html|react|solid`, `weft import-html|import-react|import-solid`, in `web.rs`), and `weft
//! import-cem`, a catalog from a Custom Elements Manifest (in `cem.rs`), and `weft schema`, the
//! JSON Schema of the documents a catalog admits (in `schema.rs`). The project file (`weft.json`, SPEC §10) found
//! above the document, or given with `--project`, supplies the catalog, tokens, actions and data
//! schema, and the settings of §10.6 (`validate.mode`, `format.write`); a flag overrides the
//! project, which overrides the default. `--catalog` replaces the project's catalog. Without
//! either, `validate` checks the syntax and shape layers. Exit codes: 0 ok, 1 diagnostics with errors,
//! 2 usage or I/O failure.

mod a2ui;
mod cem;
mod convert;
mod schema;
mod slint;
mod swiftui;
mod web;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use weft_catalog::{PROJECT_FILE, Project, ProjectOptions, load_project_text, token_types};
use weft_core::{
    Catalog, DataCheckOptions, Diagnostic, Document, Mode, ParseOptions, ValidateOptions,
    check_data, check_data_json, explain, explain_changes, has_errors, parse, parse_json,
    serialize, validate,
};

#[derive(Parser)]
#[command(
    name = "weft",
    version,
    about = "Validate, format and explain Weft documents, and convert them to and from SwiftUI, HTML, React, SolidJS, Slint and A2UI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Which project file applies (SPEC §10.1).
#[derive(Args)]
struct ProjectArgs {
    /// Project file; without it, the nearest `weft.json` above the document is used.
    #[arg(long, conflicts_with = "no_project")]
    project: Option<PathBuf>,
    /// Ignore any project file.
    #[arg(long)]
    no_project: bool,
}

/// A generator of a web target.
#[derive(Args)]
struct WebExport {
    file: PathBuf,
    /// Catalog JSON; replaces the project's catalog (default: the core catalog).
    #[arg(long)]
    catalog: Option<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.<extension>` here instead of printing (default: the project's
    /// `export.<target>.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Keep the canonical screen in a leading comment, so the importer gives it back exactly
    /// (default: the project's `export.<target>.source`, else off).
    #[arg(long, conflicts_with = "no_source")]
    source: bool,
    /// Leave the source comment out, whatever the project says.
    #[arg(long)]
    no_source: bool,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

/// Sample data a generator shows (SPEC §9).
#[derive(Args)]
struct SampleData {
    /// JSON the bindings read, shown in the output (default: the project's
    /// `export.<target>.data`, else none).
    #[arg(long)]
    data: Option<PathBuf>,
}

/// What the React and SolidJS generators add.
#[derive(Args)]
struct JsxExport {
    #[command(flatten)]
    common: WebExport,
    /// TSX with typed props (default: the project's `export.<target>.typescript`, else JSX).
    #[arg(long, conflicts_with = "javascript")]
    typescript: bool,
    /// JSX, whatever the project says.
    #[arg(long)]
    javascript: bool,
}

/// An importer of a web target.
#[derive(Args)]
struct WebImport {
    file: PathBuf,
    /// Catalog JSON; replaces the project's catalog (default: the core catalog).
    #[arg(long)]
    catalog: Option<PathBuf>,
    /// Token JSON or a DTCG resolver the page's custom properties are matched against; replaces the project's tokens
    /// (default: the default tokens).
    #[arg(long)]
    tokens: Option<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.weft` here instead of printing (default: the project's
    /// `import.<target>.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Check a document: markup, or canonical JSON when the file name ends in `.json`.
    Validate {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog.
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Report unknown content as errors instead of warnings (default: the project's
        /// `validate.mode`, else lenient).
        #[arg(long, conflicts_with = "lenient")]
        strict: bool,
        /// Report unknown content as warnings, whatever the project says.
        #[arg(long)]
        lenient: bool,
    },
    /// Print the canonical markup of a document.
    Fmt {
        file: PathBuf,
        /// Rewrite the file instead of printing; untouched when already canonical (default: the
        /// project's `format.write`, else print).
        #[arg(long, conflicts_with = "print")]
        write: bool,
        /// Print, whatever the project says.
        #[arg(long)]
        print: bool,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Read back what each binding, token, event and loop of a markup document means, one per
    /// line, so the meaning can be compared with the instruction behind an edit.
    Explain {
        file: PathBuf,
        /// An older version of the document: print only what was added, removed or changed.
        #[arg(long)]
        against: Option<PathBuf>,
        /// Catalog JSON; replaces the project's catalog. Without either, boolean and writable
        /// props read as plain reads.
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Generate a SwiftUI view (iOS 17, macOS 14) from a markup document.
    Swiftui {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog (default: the core catalog).
        #[arg(long)]
        catalog: Option<PathBuf>,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.swift` here instead of printing (default: the project's
        /// `export.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[command(flatten)]
        data: SampleData,
        /// Read the tokens from the shared `WeftTokens` that `weft swiftui-tokens` writes
        /// (default: the project's `export.swiftui.sharedTokens`, which is true; without a project,
        /// the screen carries its own tokens).
        #[arg(long, conflicts_with = "no_shared_tokens")]
        shared_tokens: bool,
        /// Put the tokens the screen uses in the screen file, so it builds on its own.
        #[arg(long)]
        no_shared_tokens: bool,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate `WeftTokens.swift`, the design tokens every SwiftUI screen of a project shares.
    SwiftuiTokens {
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `WeftTokens.swift` here instead of printing (default: the project's
        /// `export.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read a SwiftUI view back into markup; what Weft cannot hold is listed on stderr as losses.
    ImportSwiftui {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog (default: the core catalog).
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.weft` here instead of printing (default: the project's
        /// `import.swiftui.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate a Slint component (Slint 1.x, `std-widgets.slint`) from a markup document.
    Slint {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog (default: the core catalog).
        #[arg(long)]
        catalog: Option<PathBuf>,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Component name (`<Name>Screen`); the screen id when absent.
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.slint` here instead of printing (default: the project's
        /// `export.slint.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read a generated Slint component back into markup.
    ImportSlint {
        file: PathBuf,
        /// Catalog JSON; replaces the project's catalog (default: the core catalog).
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.weft` here instead of printing (default: the project's
        /// `import.slint.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate a static HTML page with CSS and no script from a markup document.
    Html {
        #[command(flatten)]
        common: WebExport,
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        #[command(flatten)]
        data: SampleData,
    },
    /// Generate a React component (JSX or TSX) from a markup document.
    React(JsxExport),
    /// Generate a SolidJS component (JSX or TSX) from a markup document.
    Solid(JsxExport),
    /// Generate a Lit web component (JavaScript) from a markup document.
    Lit(WebExport),
    /// Generate `weft-tokens.css`, the design tokens React, SolidJS and Lit components read as CSS
    /// custom properties.
    ///
    /// With light and dark themes (a DTCG resolver), the dark values apply under
    /// `prefers-color-scheme: dark`.
    CssTokens {
        /// Token JSON or a DTCG resolver; replaces the project's tokens (default: the default tokens).
        #[arg(long)]
        tokens: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `weft-tokens.css` here instead of printing (default: the project's
        /// `export.css.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Generate `weft-base.css`, the base stylesheet React, SolidJS and Lit components share with the
    /// static page.
    ///
    /// Its rules read the custom properties of `weft css-tokens` and fall back to the default
    /// tokens' values, so link it after `weft-tokens.css` (or alone).
    CssBase {
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `weft-base.css` here instead of printing (default: the project's
        /// `export.css.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Read an HTML page back into markup; what Weft cannot hold is listed on stderr as losses.
    ImportHtml(WebImport),
    /// Read a React component (.jsx, or .tsx as TypeScript) back into markup; losses go to stderr.
    ImportReact(WebImport),
    /// Read a SolidJS component (.jsx, or .tsx as TypeScript) back into markup; losses go to
    /// stderr.
    ImportSolid(WebImport),
    /// Write a markup document as A2UI v0.9 messages (basic catalog); what A2UI cannot hold is
    /// listed on stderr as losses.
    A2ui(A2uiArgs),
    /// Read A2UI v0.9 messages (a JSON array, one object or JSON Lines) back into markup; losses
    /// go to stderr.
    ImportA2ui(A2uiArgs),
    /// Turn a Custom Elements Manifest into a catalog that extends the project's; losses and
    /// diagnostics go to stderr.
    ImportCem {
        /// The manifest (`custom-elements.json`, schema 2.x), at most 10 MB.
        file: PathBuf,
        /// The catalog's name (default: the project's `import.cem.name`, else the file stem).
        #[arg(long)]
        name: Option<String>,
        /// The catalog's version (default: the project's `import.cem.version`, else 0.0.0).
        #[arg(long)]
        version: Option<String>,
        /// Catalog JSON the result extends; replaces the project's catalog (default: the core
        /// catalog). Its kinds are left out of the result.
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[command(flatten)]
        project: ProjectArgs,
        /// Write `<file stem>.catalog.json` here instead of printing (default: the project's
        /// `import.cem.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
    /// Print the JSON Schema (2020-12) of the canonical JSON documents the project's catalog
    /// admits, for a model whose decoder takes a schema (SPEC §3.1).
    Schema {
        /// Catalog JSON; replaces the project's catalog (default: the core catalog).
        #[arg(long)]
        catalog: Option<PathBuf>,
        /// Project file; without it, the nearest `weft.json` in or above the working directory.
        #[arg(long, conflicts_with = "no_project")]
        project: Option<PathBuf>,
        /// Ignore any project file.
        #[arg(long)]
        no_project: bool,
        /// Write `document.schema.json` here instead of printing (default: the project's
        /// `export.schema.outDir`, else print).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Overwrite the output file when it already exists.
        #[arg(long)]
        force: bool,
    },
}

/// A conversion to or from A2UI.
#[derive(Args)]
struct A2uiArgs {
    file: PathBuf,
    /// Catalog JSON; replaces the project's catalog (default: the core catalog).
    #[arg(long)]
    catalog: Option<PathBuf>,
    #[command(flatten)]
    project: ProjectArgs,
    /// Write `<file stem>.a2ui.json` (or `.weft` when importing) here instead of printing
    /// (default: the project's `export.a2ui.outDir` or `import.a2ui.outDir`, else print).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Overwrite the output file when it already exists.
    #[arg(long)]
    force: bool,
}

impl From<A2uiArgs> for a2ui::Args {
    fn from(a: A2uiArgs) -> Self {
        a2ui::Args {
            file: a.file,
            catalog: a.catalog,
            project: a.project,
            out_dir: a.out_dir,
            force: a.force,
        }
    }
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

/// Writes `contents` by renaming a sibling temp file into `path`, so a crash cannot leave a
/// half-written target (SPEC §10 tools that rewrite a file).
fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let name = path.file_name().unwrap_or_default();
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        std::fs::write(&tmp, contents)?;
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.with_context(|| format!("cannot write {}", path.display()))
}

/// SPEC §10.2: after the name is accepted, the resolved path (symbolic links followed) must stay
/// inside `dir`. Anything else is unreadable (`W704`).
pub(crate) fn read_project_member(dir: &Path, name: &str) -> Option<String> {
    let root = std::fs::canonicalize(dir).ok()?;
    let resolved = std::fs::canonicalize(root.join(name)).ok()?;
    resolved
        .ancestors()
        .any(|p| p == root)
        .then(|| std::fs::read_to_string(resolved).ok())
        .flatten()
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

/// The project file that applies to `document`: the one given, else the nearest one.
fn project_file(document: &Path, args: ProjectArgs) -> Option<PathBuf> {
    if args.no_project {
        None
    } else {
        args.project.or_else(|| find_project(document))
    }
}

/// The project that applies to `document`, if any, after printing its diagnostics to `out`.
/// Returns whether those diagnostics hold an error too.
fn project_for(
    document: &Path,
    args: ProjectArgs,
    mode: Mode,
    out: &mut dyn Write,
) -> Result<(Option<Project>, bool)> {
    let Some(file) = project_file(document, args) else {
        return Ok((None, false));
    };
    let (project, diagnostics) = load_project(&file, mode)?;
    print(&file, &diagnostics, out)?;
    Ok((Some(project), has_errors(&diagnostics)))
}

/// A setting of the project's §10.6 sections, when there is a project and it sets one.
fn setting<'a>(project: Option<&'a Project>, path: &[&str]) -> Option<&'a serde_json::Value> {
    project.and_then(|p| p.setting(path))
}

/// The project and its diagnostics. Files it names are read relative to its directory; the
/// loader has already refused names that leave it.
fn load_project(file: &Path, mode: Mode) -> Result<(Project, Vec<Diagnostic>)> {
    let text = read(file)?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let read_member = |name: &str| read_project_member(dir, name);
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
        Command::Fmt {
            file,
            write,
            print: print_only,
            project,
        } => {
            // Project problems go to stderr: stdout carries the formatted document.
            let (project, _) = project_for(&file, project, Mode::Lenient, &mut std::io::stderr())?;
            let write = write
                || (!print_only
                    && setting(project.as_ref(), &["format", "write"])
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false));
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
                write_atomic(&file, &formatted)?;
            }
            Ok(0)
        }
        Command::Validate {
            file,
            catalog,
            project,
            strict,
            lenient,
        } => {
            let text = read(&file)?;
            let flag = if strict {
                Some(Mode::Strict)
            } else if lenient {
                Some(Mode::Lenient)
            } else {
                None
            };
            let (project, project_failed) =
                project_for(&file, project, flag.unwrap_or_default(), out)?;
            let mode = flag.unwrap_or(
                match setting(project.as_ref(), &["validate", "mode"])
                    .and_then(serde_json::Value::as_str)
                {
                    Some("strict") => Mode::Strict,
                    _ => Mode::Lenient,
                },
            );
            let explicit = catalog.as_deref().map(load_catalog).transpose()?;
            let catalog = explicit.or_else(|| project.as_ref().map(|p| p.catalog.clone()));
            if catalog.is_none() {
                eprintln!(
                    "weft: no --catalog and no project; only the syntax and shape layers were checked"
                );
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
                    // Without a catalog the library still checks the canonical
                    // JSON shape (W200): `--no-project broken.json` must not
                    // exit 0 where the same document through the library, the
                    // MCP server or the markup path gets diagnostics (T63).
                    None => validate(
                        &json,
                        &ValidateOptions {
                            catalog: None,
                            mode,
                            tokens: tokens.as_ref(),
                            actions,
                        },
                    ),
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
            Ok(if has_errors(&diagnostics) || project_failed {
                DIAGNOSTICS
            } else {
                0
            })
        }
        Command::Explain {
            file,
            against,
            catalog,
            project,
        } => {
            // Project problems go to stderr: stdout carries the read-back.
            let (project, _) = project_for(&file, project, Mode::Lenient, &mut std::io::stderr())?;
            let explicit = catalog.as_deref().map(load_catalog).transpose()?;
            let catalog = explicit.or_else(|| project.map(|p| p.catalog));
            if catalog.is_none() {
                eprintln!(
                    "weft: no --catalog and no project; boolean and writable props read as plain reads"
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
        Command::Swiftui {
            file,
            catalog,
            tokens,
            project,
            out_dir,
            data,
            shared_tokens,
            no_shared_tokens,
            force,
        } => swiftui::export(
            swiftui::ExportArgs {
                file,
                catalog,
                tokens,
                project,
                out_dir,
                data: data.data,
                shared_tokens: (shared_tokens, no_shared_tokens),
                force,
            },
            out,
        ),
        Command::SwiftuiTokens {
            tokens,
            project,
            no_project,
            out_dir,
            force,
        } => swiftui::export_tokens(
            swiftui::TokensArgs {
                tokens,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
        Command::CssTokens {
            tokens,
            project,
            no_project,
            out_dir,
            force,
        } => web::export_css(
            swiftui::TokensArgs {
                tokens,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
        Command::CssBase {
            project,
            no_project,
            out_dir,
            force,
        } => web::export_css_base(
            ProjectArgs {
                project,
                no_project,
            },
            out_dir,
            force,
            out,
        ),
        Command::ImportSwiftui {
            file,
            catalog,
            project,
            out_dir,
            force,
        } => swiftui::import(
            swiftui::ImportArgs {
                file,
                catalog,
                project,
                out_dir,
                force,
            },
            out,
        ),
        Command::Slint {
            file,
            catalog,
            tokens,
            name,
            project,
            out_dir,
            force,
        } => slint::export(
            slint::ExportArgs {
                file,
                catalog,
                tokens,
                name,
                project,
                out_dir,
                force,
            },
            out,
        ),
        Command::ImportSlint {
            file,
            catalog,
            project,
            out_dir,
            force,
        } => slint::import(
            slint::ImportArgs {
                file,
                catalog,
                project,
                out_dir,
                force,
            },
            out,
        ),
        Command::Html {
            common,
            tokens,
            data,
        } => web_export(web::Target::Html, common, tokens, data.data, None, out),
        Command::React(args) => {
            let target = web::Target::React;
            web_export(
                target,
                args.common,
                None,
                None,
                Some((args.typescript, args.javascript)),
                out,
            )
        }
        Command::Solid(args) => {
            let target = web::Target::Solid;
            web_export(
                target,
                args.common,
                None,
                None,
                Some((args.typescript, args.javascript)),
                out,
            )
        }
        Command::Lit(common) => web_export(web::Target::Lit, common, None, None, None, out),
        Command::ImportHtml(args) => web_import(web::Target::Html, args, out),
        Command::ImportReact(args) => web_import(web::Target::React, args, out),
        Command::ImportSolid(args) => web_import(web::Target::Solid, args, out),
        Command::A2ui(args) => a2ui::export(args.into(), out),
        Command::ImportA2ui(args) => a2ui::import(args.into(), out),
        Command::ImportCem {
            file,
            name,
            version,
            catalog,
            project,
            out_dir,
            force,
        } => cem::import(
            cem::Args {
                file,
                name,
                version,
                catalog,
                project,
                out_dir,
                force,
            },
            out,
        ),
        Command::Schema {
            catalog,
            project,
            no_project,
            out_dir,
            force,
        } => schema::export(
            schema::Args {
                catalog,
                project: ProjectArgs {
                    project,
                    no_project,
                },
                out_dir,
                force,
            },
            out,
        ),
    }
}

fn web_export(
    target: web::Target,
    common: WebExport,
    tokens: Option<PathBuf>,
    data: Option<PathBuf>,
    jsx: Option<(bool, bool)>,
    out: &mut dyn Write,
) -> Result<u8> {
    let (typescript, javascript) = jsx.unwrap_or_default();
    web::export(
        web::ExportArgs {
            target,
            file: common.file,
            catalog: common.catalog,
            tokens,
            project: common.project,
            out_dir: common.out_dir,
            force: common.force,
            typescript,
            javascript,
            source: common.source,
            no_source: common.no_source,
            data,
        },
        out,
    )
}

fn web_import(target: web::Target, args: WebImport, out: &mut dyn Write) -> Result<u8> {
    web::import(
        web::ImportArgs {
            target,
            file: args.file,
            catalog: args.catalog,
            tokens: args.tokens,
            project: args.project,
            out_dir: args.out_dir,
            force: args.force,
        },
        out,
    )
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
