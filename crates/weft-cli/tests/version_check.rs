//! `weft version-check` reports W810 when the declared version is too low (SPEC §8).

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

fn weft(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(args)
        .output()
        .unwrap()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("weft-version-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, markup: &str) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, markup).unwrap();
        path.display().to_string()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_low_fragment_version_exits_with_w810_at_version() {
    let dir = Scratch::new("low");
    let old = dir.write(
        "old.weft",
        r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#,
    );
    let new = dir.write(
        "new.weft",
        r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><param name="note" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#,
    );
    let out = weft(&["version-check", &old, &new, "--no-project"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("major"), "{stdout}");
    assert!(stdout.contains("least\t2.0.0"), "{stdout}");
    assert!(stdout.contains("W810"), "{stdout}");
    assert!(stdout.contains(":1:23 "), "{stdout}");
}

#[test]
fn a_sufficient_version_exits_zero() {
    let dir = Scratch::new("ok");
    let old = dir.write(
        "old.weft",
        r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><text id="t">Hi</text></screen>"#,
    );
    let new = dir.write(
        "new.weft",
        r#"<screen id="s" label="S" version="1.0.1" weft="0.3"><text id="t">Hello</text></screen>"#,
    );
    let out = weft(&["version-check", &old, &new, "--no-project"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("patch"), "{stdout}");
    assert!(stdout.contains("least\t1.0.1"), "{stdout}");
}
