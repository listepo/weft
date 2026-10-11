//! `weft version-check`: how far a screen, fragment or library catalog must raise its version.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value as Json;
use weft_catalog::{VersionCheck, check_catalogs, check_documents};
use weft_core::{
    Catalog, Code, Diagnostic, Document, ParseOptions, ValidateOptions, parse, parse_json,
    to_document, validate,
};

use crate::print;

enum Input {
    // A document is much larger than a catalog JSON value; boxing keeps the enum small.
    Document(Box<Document>),
    Catalog(Json),
}

pub(crate) fn run(old: &Path, new: &Path, catalog: &Catalog, out: &mut dyn Write) -> Result<u8> {
    let Some(previous) = read_input(old)? else {
        return Ok(1);
    };
    let Some(next) = read_input(new)? else {
        return Ok(1);
    };
    let (check, diagnostic) = match (previous, next) {
        (Input::Document(old), Input::Document(new)) => {
            let check = check_documents(&old, &new, catalog);
            let diagnostic = (!check.ok).then(|| version_diagnostic(&new, &check));
            (check, diagnostic)
        }
        (Input::Catalog(old), Input::Catalog(new)) => {
            let check = check_catalogs(&old, &new, catalog);
            let diagnostic = (!check.ok).then(|| {
                Diagnostic::new(
                    Code::W810,
                    "#/version",
                    message(
                        check.least.as_deref(),
                        new.get("version").and_then(Json::as_str),
                    ),
                    check.least.clone().unwrap_or_default(),
                )
                .hint(hint(check.least.as_deref()))
            });
            (check, diagnostic)
        }
        _ => {
            anyhow::bail!("version-check compares two documents or two library catalogs");
        }
    };
    for change in &check.changes {
        writeln!(
            out,
            "{}\t{}\t{}",
            change.level.as_str(),
            change.path,
            change.message
        )?;
    }
    match &check.least {
        Some(least) => writeln!(out, "least\t{least}")?,
        None => writeln!(out, "least\tnone")?,
    }
    if let Some(diagnostic) = diagnostic {
        print(new, std::slice::from_ref(&diagnostic), out)?;
        return Ok(1);
    }
    Ok(0)
}

fn read_input(path: &Path) -> Result<Option<Input>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    if let Ok(json) = parse_json(&text) {
        if json.get("components").is_some() && json.get("root").is_none() {
            return Ok(Some(Input::Catalog(json)));
        }
        if json.get("root").is_some() || json.get("weft").is_some() {
            let issues = validate(&json, &ValidateOptions::default());
            if issues.iter().any(|d| d.code == Code::W200) {
                print(path, &issues, &mut std::io::stderr().lock())?;
                return Ok(None);
            }
            return Ok(Some(Input::Document(Box::new(to_document(&json)))));
        }
    }
    let parsed = parse(&text, &ParseOptions::default());
    match parsed.document {
        Some(document) => Ok(Some(Input::Document(Box::new(document)))),
        None => {
            print(path, &parsed.diagnostics, &mut std::io::stderr().lock())?;
            Ok(None)
        }
    }
}

fn version_diagnostic(document: &Document, check: &VersionCheck) -> Diagnostic {
    let path = match &document.root.id {
        Some(id) => format!("/{}#{id}/@version", document.root.kind),
        None => format!("/{}@version", document.root.kind),
    };
    let pos = document
        .root
        .source
        .0
        .as_ref()
        .and_then(|source| source.attrs.get("version").copied());
    Diagnostic::new(
        Code::W810,
        path,
        message(check.least.as_deref(), document.version.as_deref()),
        check.least.clone().unwrap_or_default(),
    )
    .hint(hint(check.least.as_deref()))
    .pos(pos)
}

fn message(least: Option<&str>, declared: Option<&str>) -> String {
    let least = least.unwrap_or("a version");
    match declared {
        Some(declared) => {
            format!("Version {declared} is lower than {least}, which the changes require.")
        }
        None => format!("The document declares no version; the changes require {least}."),
    }
}

fn hint(least: Option<&str>) -> String {
    match least {
        Some(least) => format!("write version=\"{least}\""),
        None => "remove version".to_owned(),
    }
}
