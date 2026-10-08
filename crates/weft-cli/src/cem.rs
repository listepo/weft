//! `weft import-cem` (SPEC §9, "From a Custom Elements Manifest"): a manifest file to a catalog
//! extension. The base catalog resolves as for the other importers (`--catalog`, else the
//! project's, else the core one), and so does the output (`--out-dir`, else `import.cem.outDir`,
//! else standard output); the catalog's name and version come from the flags, else
//! `import.cem.name` and `import.cem.version`, else the file stem and `0.0.0`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use weft_core::has_errors;
use weft_import::{CemOptions, MAX_MANIFEST_LENGTH, import_cem};

use crate::convert::{catalog, emit, out_dir, project, report_losses};
use crate::{DIAGNOSTICS, ProjectArgs, print};

pub struct Args {
    pub file: PathBuf,
    pub name: Option<String>,
    pub version: Option<String>,
    pub catalog: Option<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    pub force: bool,
}

pub fn import(args: Args, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let base = catalog(args.catalog.as_deref(), project)?;
    let text = read_bounded(&args.file)?;
    let setting = |key: &str| {
        project
            .and_then(|p| p.setting(&["import", "cem", key]))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let name = match args.name.or_else(|| setting("name")) {
        Some(name) => name,
        None => args
            .file
            .file_stem()
            .with_context(|| format!("{} has no file name", args.file.display()))?
            .to_string_lossy()
            .into_owned(),
    };
    let version = args
        .version
        .or_else(|| setting("version"))
        .unwrap_or_else(|| "0.0.0".to_owned());
    let result = import_cem(
        &text,
        &CemOptions {
            name: &name,
            version: &version,
            base: &base,
        },
    );
    let errors = &mut std::io::stderr();
    print(&args.file, &result.diagnostics, errors)?;
    report_losses(&args.file, &result.losses, errors)?;
    let mut json = serde_json::to_string_pretty(&result.catalog)?;
    json.push('\n');
    let dir = out_dir(
        args.out_dir,
        project,
        project_dir.as_deref(),
        "import",
        "cem",
    );
    // Not `.json` alone: beside the manifest it would overwrite `custom-elements.json` itself.
    emit(&json, &args.file, dir, "catalog.json", args.force, out)?;
    Ok(if has_errors(&result.diagnostics) {
        DIAGNOSTICS
    } else {
        0
    })
}

/// At most one byte past the limit is read, so an oversized manifest, or one that grows while it
/// is read, never reaches memory whole.
fn read_bounded(file: &Path) -> Result<String> {
    let cannot = || format!("cannot read {}", file.display());
    let limit = u64::try_from(MAX_MANIFEST_LENGTH)?;
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .with_context(cannot)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .with_context(cannot)?;
    if bytes.len() > MAX_MANIFEST_LENGTH {
        bail!(
            "{} is longer than {MAX_MANIFEST_LENGTH} bytes, the limit of a manifest; it was not read",
            file.display()
        );
    }
    String::from_utf8(bytes).with_context(cannot)
}
