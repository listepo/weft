//! The project on disk: discovery above a document, reads that stay inside the project, the
//! file-reading loader and the atomic write.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_catalog::{
    ProjectFileError, WriteError, find_project, load_project_file, read_project_member,
    write_atomic,
};
use weft_core::{Mode, has_errors};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Canonical, so paths compare equal where the temporary directory is a symbolic link.
        Scratch(std::fs::canonicalize(dir).unwrap())
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_nearest_project_above_the_document_is_found() {
    let scratch = Scratch::new("find");
    let outer = scratch.write("weft.json", "{}");
    let inner = scratch.write("app/weft.json", "{}");
    assert_eq!(
        find_project(&scratch.0.join("app/screens/a.weft")),
        Some(inner)
    );
    assert_eq!(find_project(&scratch.0.join("b.weft")), Some(outer));
}

#[test]
fn a_member_outside_the_project_is_not_read() {
    let scratch = Scratch::new("member");
    scratch.write("project/tokens.json", "{}");
    scratch.write("secret.json", "{}");
    let dir = scratch.0.join("project");
    assert_eq!(
        read_project_member(&dir, "tokens.json").as_deref(),
        Some("{}")
    );
    assert_eq!(read_project_member(&dir, "../secret.json"), None);
}

#[test]
fn the_loader_reads_members_and_keeps_their_problems() {
    let scratch = Scratch::new("load");
    let file = scratch.write(
        "weft.json",
        r#"{ "actions": ["a.b"], "catalog": "missing.json" }"#,
    );
    let loaded = load_project_file(&file, Mode::Lenient).unwrap();
    assert_eq!(loaded.project.actions, Some(vec!["a.b".to_owned()]));
    assert!(has_errors(&loaded.diagnostics));
    let missing = scratch.0.join("absent/weft.json");
    let error = load_project_file(&missing, Mode::Lenient).unwrap_err();
    assert!(matches!(&error, ProjectFileError::Read(path, _) if *path == missing));
    assert_eq!(
        error.to_string(),
        format!("cannot read {}", missing.display())
    );
}

#[test]
fn a_new_file_is_written_and_a_failed_write_leaves_no_temporary_file() {
    let scratch = Scratch::new("write");
    let file = scratch.0.join("new.weft");
    write_atomic(&file, "one").unwrap();
    write_atomic(&file, "two").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
    scratch.write("dir/keep.txt", "kept");
    let error = write_atomic(&scratch.0.join("dir"), "three").unwrap_err();
    assert!(matches!(error, WriteError::Write(..)), "{error:?}");
    let mut names: Vec<_> = std::fs::read_dir(&scratch.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["dir", "new.weft"]);
}
