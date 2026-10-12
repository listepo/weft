//! The project on disk (SPEC §10.1–§10.2): finding the project file above a document, reading the
//! files it names without leaving its directory, loading it, and the atomic write that the tools
//! which rewrite a file share. The rest of the crate stays pure; only this module touches the file
//! system.

use std::path::{Path, PathBuf};

use weft_core::Mode;

use crate::{
    CatalogError, PROJECT_FILE, ProjectLoad, ProjectOptions, load_project_text, split_package_read,
};

#[derive(Debug, thiserror::Error)]
pub enum ProjectFileError {
    #[error("cannot read {}", .0.display())]
    Read(PathBuf, #[source] std::io::Error),
    #[error("the embedded core catalog is broken")]
    Embedded(#[source] CatalogError),
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("cannot resolve {}", .0.display())]
    Resolve(PathBuf, #[source] std::io::Error),
    #[error("cannot stat {}", .0.display())]
    Stat(PathBuf, #[source] std::io::Error),
    #[error("cannot write {}", .0.display())]
    Write(PathBuf, #[source] std::io::Error),
}

/// SPEC §10.1: the first `weft.json` in the document's directory or one above it.
pub fn find_project(document: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(document).ok()?;
    absolute
        .ancestors()
        .skip(1)
        .map(|dir| dir.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

/// SPEC §10.2: after the name is accepted, the resolved path (symbolic links followed) must stay
/// inside `dir`. Anything else is unreadable (`W704`).
pub fn read_project_member(dir: &Path, name: &str) -> Option<String> {
    let root = std::fs::canonicalize(dir).ok()?;
    let resolved = std::fs::canonicalize(root.join(name)).ok()?;
    resolved
        .ancestors()
        .any(|p| p == root)
        .then(|| std::fs::read_to_string(resolved).ok())
        .flatten()
}

/// SPEC §10.2 packages: the package is `node_modules/<package>` of the project directory or of the
/// nearest parent that has it, as Node resolves one, and only files whose resolved path stays
/// inside that package directory are read. Nothing is run and nothing is fetched.
fn read_package_file(project_dir: &Path, package: &str, file: &str) -> Option<String> {
    let start = if project_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        project_dir
    };
    let package_dir = std::path::absolute(start)
        .ok()?
        .ancestors()
        .map(|dir| dir.join("node_modules").join(package))
        .find(|dir| dir.join("package.json").is_file())?;
    read_project_member(&package_dir, file)
}

/// A file the project names: one of its own, or one of an npm package the loader asks for.
fn read_member(dir: &Path, name: &str) -> Option<String> {
    match split_package_read(name) {
        Some((package, file)) => read_package_file(dir, package, file),
        None => read_project_member(dir, name),
    }
}

/// The project in `file` and its diagnostics. Files it names are read relative to its directory;
/// the loader has already refused names that leave it.
pub fn load_project_file(file: &Path, mode: Mode) -> Result<ProjectLoad, ProjectFileError> {
    let text =
        std::fs::read_to_string(file).map_err(|e| ProjectFileError::Read(file.to_owned(), e))?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let read_member = |name: &str| read_member(dir, name);
    load_project_text(
        &text,
        &ProjectOptions {
            read: Some(&read_member),
            mode,
            ..Default::default()
        },
    )
    .map_err(ProjectFileError::Embedded)
}

/// Writes `contents` by renaming a sibling temp file into the file `path` names, so a crash cannot
/// leave a half-written target (SPEC §10 tools that rewrite a file). A symbolic link is followed
/// to its target and left in place; the target's permissions are kept. A target that does not
/// exist yet is created with the default permissions.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), WriteError> {
    let target = if path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        std::fs::canonicalize(path).map_err(|e| WriteError::Resolve(path.to_owned(), e))?
    } else {
        path.to_path_buf()
    };
    let permissions = match std::fs::metadata(&target) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(WriteError::Stat(target, e)),
    };
    let name = target.file_name().unwrap_or_default();
    let tmp = target.with_file_name(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        std::fs::write(&tmp, contents)?;
        if let Some(permissions) = permissions {
            std::fs::set_permissions(&tmp, permissions)?;
        }
        std::fs::rename(&tmp, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written.map_err(|e| WriteError::Write(path.to_owned(), e))
}
