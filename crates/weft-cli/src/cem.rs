//! `weft import-cem` (SPEC §9, "From a Custom Elements Manifest"): a manifest file to a catalog
//! extension. The base catalog resolves as for the other importers (`--catalog`, else the
//! project's, else the core one), and so does the output (`--out-dir`, else `import.cem.outDir`,
//! else standard output); the catalog's name, version and prefix come from the flags, else
//! `import.cem.name`, `import.cem.version` and `import.cem.prefix`, else the file stem, `0.0.0`
//! and no prefix.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use weft_catalog::core_catalog;
use weft_core::{Catalog, has_errors, is_name};
use weft_import::{CemOptions, MAX_MANIFEST_LENGTH, import_cem};

use crate::convert::{catalog, emit, out_dir, project, report_losses};
use crate::{DIAGNOSTICS, ProjectArgs, print};

pub struct Args {
    pub file: PathBuf,
    pub name: Option<String>,
    pub version: Option<String>,
    pub prefix: Option<String>,
    pub catalog: Vec<PathBuf>,
    pub project: ProjectArgs,
    pub out_dir: Option<PathBuf>,
    pub force: bool,
}

pub fn import(args: Args, out: &mut dyn Write) -> Result<u8> {
    let (project, project_dir) = project(&args.file, args.project)?;
    let project = project.as_ref();
    let base = catalog(&args.catalog, project)?;
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
    let prefix = args.prefix.or_else(|| setting("prefix"));
    if let Some(prefix) = &prefix
        && (!is_name(prefix) || prefix.contains('-'))
    {
        bail!("the prefix {prefix:?} is not one lowercase name segment such as \"acme\"");
    }
    let core = core_catalog().context("the embedded core catalog is broken")?;
    let result = import_cem(
        &text,
        &CemOptions {
            name: &name,
            version: &version,
            base: &base,
            prefix: prefix.as_deref(),
            core_version: &core.version,
        },
    );
    let errors = &mut std::io::stderr();
    print(&args.file, &result.diagnostics, errors)?;
    report_losses(&args.file, &result.losses, errors)?;
    if prefix.is_none()
        && let Some(segment) = shared_segment(&result.catalog)
    {
        eprintln!(
            "weft: every kind starts with \"{segment}-\"; with --prefix {segment} the catalog is a library that loads beside a project catalog (SPEC §10.4)"
        );
    }
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

/// The first name segment every kind shares, which a library would declare as its prefix.
fn shared_segment(catalog: &Catalog) -> Option<&str> {
    let mut kinds = catalog.components.keys();
    let (segment, _) = kinds.next()?.split_once('-')?;
    kinds
        .all(|k| k.split_once('-').is_some_and(|(first, _)| first == segment))
        .then_some(segment)
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
