// The command plugin: `swift package weft export|import`. It must ask for write permission, write
// where the project says, and round-trip a corpus screen.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { run, skipReason, weftBinary, workspace } from "./helpers.ts";

describe.skipIf(skipReason)("command plugin", () => {
  const ws = workspace("PackageSample");
  const scratch = join(ws.root, "scratch");
  const swiftPackage = (args: string[], allowWrite = true) =>
    run(
      "swift",
      [
        "package",
        "--scratch-path",
        scratch,
        ...(allowWrite ? ["--allow-writing-to-package-directory"] : []),
        "weft",
        ...args,
      ],
      ws.sample,
    );
  const screen = "Sources/Screens/login.weft";
  afterAll(ws.cleanup);

  test("asks for permission to write into the package", { timeout: 300_000 }, () => {
    const refused = swiftPackage(["export", screen], false);
    expect(refused.status).not.toBe(0);
    expect(refused.output).toContain("--allow-writing-to-package-directory");
    expect(existsSync(join(ws.sample, "Sources/Screens/login.swift"))).toBe(false);
  });

  test("a corpus screen survives export and import", { timeout: 300_000 }, () => {
    const exported = swiftPackage(["export", screen, "--out-dir", "Generated"]);
    expect(exported.status, exported.output).toBe(0);
    const swift = join(ws.sample, "Generated/login.swift");
    expect(readFileSync(swift, "utf8")).toContain("var theme = WeftTokens()");
    // The screen reads the project's shared tokens, so they are written next to it.
    expect(readFileSync(join(ws.sample, "Generated/WeftTokens.swift"), "utf8")).toContain(
      "struct WeftTokens: Sendable",
    );

    const imported = swiftPackage(["import", "Generated/login.swift", "--out-dir", "Imported"]);
    expect(imported.status, imported.output).toBe(0);
    // The importer reads back exactly what the generator printed (SPEC §9), so the result is the
    // canonical form of the original screen.
    const canonical = run(weftBinary(), ["fmt", join(ws.sample, screen)], ws.sample).output;
    // The importer stamps the current version; the corpus screen stays at 0.1.
    const current = canonical.replace('weft="0.1"', 'weft="0.2"');
    expect(readFileSync(join(ws.sample, "Imported/login.weft"), "utf8")).toBe(current);
  });

  test("refuses to replace a file unless told to", { timeout: 300_000 }, () => {
    const again = swiftPackage(["export", screen, "--out-dir", "Generated"]);
    expect(again.status).not.toBe(0);
    expect(again.output).toContain("--force");
    const forced = swiftPackage(["export", screen, "--out-dir", "Generated", "--force"]);
    expect(forced.status, forced.output).toBe(0);
  });

  test("writes next to the input, or where weft.json says", { timeout: 300_000 }, () => {
    const next = swiftPackage(["export", screen]);
    expect(next.status, next.output).toBe(0);
    expect(existsSync(join(ws.sample, "Sources/Screens/login.swift"))).toBe(true);

    const project = join(ws.sample, "weft.json");
    writeFileSync(
      project,
      JSON.stringify({
        ...JSON.parse(readFileSync(project, "utf8")),
        export: { swiftui: { outDir: "ios" } },
      }),
    );
    mkdirSync(join(ws.sample, "ios"), { recursive: true });
    const configured = swiftPackage(["export", screen]);
    expect(configured.status, configured.output).toBe(0);
    expect(existsSync(join(ws.sample, "ios/login.swift"))).toBe(true);
  });

  test("rejects a wrong extension and a missing subcommand", { timeout: 300_000 }, () => {
    const wrong = swiftPackage(["export", "Package.swift"]);
    expect(wrong.status).not.toBe(0);
    expect(wrong.output).toContain("expected a .weft file");
    const none = swiftPackage([]);
    expect(none.status).not.toBe(0);
    expect(none.output).toContain("usage:");
  });
});
