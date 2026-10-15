// MSBuild refuses a project file whose XML is not well formed, but only on Windows, where nothing
// builds by hand; during restore it even skips the broken import silently and fails later with a
// misleading error. A double hyphen inside a comment is the mistake that slipped through once
// (a command-line flag quoted in a comment), so every OS checks for it here.
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, test } from "vitest";
import { repoRoot } from "../scripts/slint-bindings.ts";

const windows = join(repoRoot, "studio/windows");
const xml = /\.(props|targets|csproj|xaml|manifest)$/;
const skipped = new Set(["bin", "obj"]);

const xmlFiles = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return skipped.has(entry.name) ? [] : xmlFiles(path);
    return xml.test(entry.name) ? [path] : [];
  });

/** Comments whose body holds `--`, which XML 1.0 (§2.5) forbids. */
const badComments = (text: string): string[] =>
  [...text.matchAll(/<!--([\s\S]*?)-->/g)].map((m) => m[1] ?? "").filter((body) => body.includes("--"));

describe("studio/windows MSBuild and XAML files", () => {
  test("the check finds a double hyphen inside a comment", () => {
    expect(badComments("<!-- run `x --msbuild` -->\n<!-- fine -->")).toEqual([" run `x --msbuild` "]);
  });

  test("no comment holds a double hyphen", () => {
    const files = xmlFiles(windows);
    expect(files.length).toBeGreaterThan(0);
    const bad = files.flatMap((file) =>
      badComments(readFileSync(file, "utf8")).map((body) => `${relative(repoRoot, file)}: <!--${body}-->`),
    );
    expect(bad).toEqual([]);
  });
});
