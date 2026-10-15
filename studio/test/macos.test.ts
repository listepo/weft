// The macOS shell (T69.1d): XcodeGen, `xcodebuild build` of the app, which builds
// libweft_studio_ffi.a with Cargo, then `xcodebuild test`: the document model with a fake picker
// and the UI test that opens a corpus screen in the window. Apple Silicon with Xcode and XcodeGen
// only; elsewhere it skips and says why.
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { afterAll, describe, expect, test } from "vitest";
import { generateProject, macos } from "../macos/scripts/xcodegen.ts";

const run = (command: string, args: string[]) => {
  const result = spawnSync(command, args, { cwd: macos, encoding: "utf8", maxBuffer: 1 << 28 });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
};

const works = (command: string, args: string[]): boolean => {
  try {
    return spawnSync(command, args, { stdio: "ignore" }).status === 0;
  } catch {
    return false;
  }
};

const skipReason = (() => {
  if (process.platform !== "darwin") return "not macOS";
  // T69.1: the app is arm64 only, like every macOS build of the workspace.
  if (process.arch !== "arm64") return "not Apple Silicon";
  if (!works("xcrun", ["--find", "xcodebuild"])) return "xcodebuild is not available (no Xcode)";
  if (!works("xcodegen", ["--version"])) return "xcodegen is not installed";
  if (!works("cargo", ["--version"])) return "cargo is not available";
  return undefined;
})();
if (skipReason) {
  console.warn(`skipped: the macOS shell tests need Apple Silicon with Xcode: ${skipReason}`);
}

describe.skipIf(skipReason)("studio/macos", () => {
  const derived = mkdtempSync(`${tmpdir()}/weft-studio-macos-`);
  afterAll(() => rmSync(derived, { recursive: true, force: true }));

  const xcodebuild = (action: string) =>
    run("xcodebuild", [
      "-project",
      "WeftStudio.xcodeproj",
      "-scheme",
      "WeftStudio",
      "-destination",
      "platform=macOS,arch=arm64",
      "-derivedDataPath",
      derived,
      action,
    ]);

  test("XcodeGen and xcodebuild build of the app", { timeout: 1_800_000 }, () => {
    const generated = generateProject();
    expect(generated.status, generated.output).toBe(0);
    const built = xcodebuild("build");
    expect(built.status, built.output.slice(-8000)).toBe(0);
  });

  test("xcodebuild test: the document model and the UI test", { timeout: 1_800_000 }, () => {
    const tested = xcodebuild("test");
    expect(tested.status, tested.output.slice(-8000)).toBe(0);
    expect(tested.output).toContain("testACorpusScreenOpensDrawsAndSavesAs");
  });
});
