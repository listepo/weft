// The Source Editor Extension: its conversion core is unit-tested with `swift test`, and the
// project builds with xcodebuild, signed ad hoc, with a sandboxed helper that can run. What needs
// Xcode's own UI (enabling the extension, the Editor menu) is listed in docs/xcode-plugin.md.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { hasXcodegen, pluginDir, remove, run, skipReason, tempDir } from "./helpers.ts";

const editor = join(pluginDir, "editor-extension");

describe.skipIf(skipReason)("editor extension: conversion core", () => {
  const scratch = tempDir("editor-core");
  afterAll(() => remove(scratch));

  test("swift test passes", { timeout: 600_000 }, () => {
    const result = run("swift", ["test", "--scratch-path", scratch], join(editor, "Core"));
    expect(result.status, result.output.slice(-4000)).toBe(0);
    expect(result.output).toMatch(/Test run with \d+ tests? in \d+ suites? passed/);
  });
});

describe.skipIf(skipReason || !hasXcodegen)("editor extension: app and appex", () => {
  const derived = tempDir("editor-app");
  const app = join(derived, "Build/Products/Release/WeftEditor.app");
  const appex = join(app, "Contents/PlugIns/WeftEditorExtension.appex");
  afterAll(() => remove(derived));

  test("builds with xcodebuild, signed ad hoc", { timeout: 900_000 }, () => {
    expect(run("xcodegen", ["generate", "--quiet"], editor).status).toBe(0);
    const built = run(
      "xcodebuild",
      [
        "-project",
        "WeftEditor.xcodeproj",
        "-scheme",
        "WeftEditor",
        "-configuration",
        "Release",
        "-destination",
        "generic/platform=macOS",
        "-derivedDataPath",
        derived,
        "ARCHS=arm64",
        "ONLY_ACTIVE_ARCH=YES",
        "build",
      ],
      editor,
    );
    expect(built.status, built.output.slice(-4000)).toBe(0);
    expect(run("codesign", ["--verify", "--deep", "--strict", app]).status).toBe(0);
    expect(run("codesign", ["-dv", app]).output).toContain("Signature=adhoc");
  });

  test("the extension declares its three commands for Xcode", () => {
    const plist = (key: string, format = "json") =>
      run("plutil", ["-extract", key, format, "-o", "-", join(appex, "Contents/Info.plist")])
        .output;
    expect(plist("NSExtension.NSExtensionPointIdentifier", "raw")).toContain(
      "com.apple.dt.Xcode.extension.source-editor",
    );
    const commands = JSON.parse(
      plist("NSExtension.NSExtensionAttributes.XCSourceEditorCommandDefinitions"),
    );
    expect(commands.map((c: Record<string, string>) => c.XCSourceEditorCommandIdentifier)).toEqual([
      "dev.weft.editor.import-selection",
      "dev.weft.editor.generate-swiftui",
      "dev.weft.editor.validate",
    ]);
  });

  test("the extension and its helper carry the sandbox entitlements", () => {
    const entitlements = (path: string) =>
      run("codesign", ["-d", "--entitlements", "-", "--xml", path]).output;
    expect(entitlements(appex)).toContain("com.apple.security.app-sandbox");
    expect(entitlements(appex)).not.toContain("com.apple.security.inherit");
    const helper = entitlements(join(appex, "Contents/MacOS/weft"));
    expect(helper).toContain("com.apple.security.app-sandbox");
    expect(helper).toContain("com.apple.security.inherit");
    // Apple: the system aborts a child that has any other entitlement.
    expect(helper.match(/<key>/g)).toHaveLength(2);
    expect(run("lipo", ["-archs", join(appex, "Contents/MacOS/weft")]).output.trim()).toBe("arm64");
  });

  test("the bundled catalog is the core catalog", () => {
    const bundled = readFileSync(join(appex, "Contents/Resources/catalog.json"), "utf8");
    expect(bundled).toBe(
      readFileSync(join(pluginDir, "../../packages/catalog/catalog.json"), "utf8"),
    );
  });

  test("the sandboxed app runs the embedded weft", { timeout: 120_000 }, () => {
    const result = run(join(app, "Contents/MacOS/WeftEditor"), ["--self-test"]);
    expect(result.output).toContain("self-test ok (sandboxed)");
    expect(result.status).toBe(0);
  });
});
