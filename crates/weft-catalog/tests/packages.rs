//! npm package names and the package reads a loader asks its reader for (SPEC §10.2).

use weft_catalog::{is_package_name, split_package_read};

#[test]
fn package_names_are_the_ones_npm_accepts() {
    for name in ["acme-ui", "@acme/ui", "a.b_c~d", "x1", &"a".repeat(214)] {
        assert!(is_package_name(name), "{name}");
    }
    let long = "a".repeat(215);
    for name in [
        "",
        ".",
        "..",
        "../x",
        "_x",
        ".x",
        "Acme",
        "@acme",
        "@acme/",
        "@/ui",
        "@acme/ui/x",
        "a/b",
        "a\\b",
        "a:b",
        "/abs",
        "a b",
        &long,
    ] {
        assert!(!is_package_name(name), "{name}");
    }
}

#[test]
fn package_reads_split_into_a_checked_package_and_file() {
    assert_eq!(
        split_package_read("package:@acme/ui/dist/weft.json"),
        Some(("@acme/ui", "dist/weft.json"))
    );
    assert_eq!(
        split_package_read("package:acme-ui/package.json"),
        Some(("acme-ui", "package.json"))
    );
    for name in [
        "acme-ui/package.json",
        "package:acme-ui",
        "package:@acme/ui",
        "package:../x/package.json",
        "package:acme-ui/../x.json",
        "package:acme-ui//etc/passwd",
        "package:@acme/../x.json",
    ] {
        assert_eq!(split_package_read(name), None, "{name}");
    }
}
