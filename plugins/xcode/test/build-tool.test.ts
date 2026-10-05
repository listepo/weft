// The build tool plugin on the SwiftPM sample: a `.weft` file becomes SwiftUI during `swift build`,
// the generated file typechecks for iOS 17 and macOS 14, and the build is incremental.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { run, skipReason, workspace } from "./helpers.ts";

const GENERATING = "Weft: generate SwiftUI from login.weft";

describe.skipIf(skipReason)("build tool plugin (SwiftPM)", () => {
  const ws = workspace("PackageSample");
  const scratch = join(ws.root, "scratch");
  const build = () => run("swift", ["build", "--scratch-path", scratch, "-v"], ws.sample);
  const generated = () =>
    join(
      scratch,
      "plugins/outputs/packagesample/Screens/destination/WeftBuildToolPlugin/Sources/Screens/login.swift",
    );
  afterAll(ws.cleanup);

  test("builds the target with the generated screen", { timeout: 600_000 }, () => {
    const first = build();
    expect(first.output).toContain(GENERATING);
    expect(first.status, first.output).toBe(0);
    const swift = readFileSync(generated(), "utf8");
    expect(swift).toContain("struct LoginScreen: View");
    // The project's token layer reached the generator through the `--project` the plugin passes.
    expect(swift).toContain("var md: CGFloat = 16");
  });

  test.each([
    ["iphonesimulator", "arm64-apple-ios17.0-simulator"],
    ["macosx", "arm64-apple-macos14.0"],
  ])("the generated file typechecks for %s", { timeout: 300_000 }, (sdk, target) => {
    expect(existsSync(generated())).toBe(true);
    const result = run("xcrun", [
      "--sdk",
      sdk,
      "swiftc",
      "-typecheck",
      "-swift-version",
      "6",
      "-target",
      target,
      generated(),
    ]);
    expect(result.status, result.output).toBe(0);
  });

  test("an unchanged build runs nothing", { timeout: 600_000 }, () => {
    const again = build();
    expect(again.status, again.output).toBe(0);
    expect(again.output).not.toContain(GENERATING);
  });

  test("a token file named by weft.json is an input", { timeout: 600_000 }, () => {
    const tokens = join(ws.sample, "tokens/brand.tokens.json");
    writeFileSync(tokens, readFileSync(tokens, "utf8").replace('"value": 16', '"value": 24'));
    const changed = build();
    expect(changed.output).toContain(GENERATING);
    expect(changed.status, changed.output).toBe(0);
    expect(readFileSync(generated(), "utf8")).toContain("var md: CGFloat = 24");
  });

  test(
    "an invalid screen fails the build with the validator's diagnostic",
    { timeout: 600_000 },
    () => {
      const screen = join(ws.sample, "Sources/Screens/login.weft");
      writeFileSync(screen, readFileSync(screen, "utf8").replace("<heading", "<headline"));
      const broken = build();
      expect(broken.status).not.toBe(0);
      expect(broken.output).toMatch(/login\.weft:\d+:\d+ W\d+/);
    },
  );
});
