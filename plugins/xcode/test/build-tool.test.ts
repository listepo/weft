// The build tool plugin on the SwiftPM sample: its `.weft` files become SwiftUI during
// `swift build`, with one `WeftTokens.swift` the screens share and a custom component the app
// writes; the generated files typecheck for iOS 17 and macOS 14, and the build is incremental.
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { run, skipReason, workspace } from "./helpers.ts";

const GENERATING = "Weft: generate SwiftUI from login.weft";
const GENERATING_TOKENS = "Weft: generate WeftTokens.swift from weft.json";

/** Every file under `dir`, as paths. */
const walk = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)],
  );

describe.skipIf(skipReason)("build tool plugin (SwiftPM)", () => {
  const ws = workspace("PackageSample");
  const scratch = join(ws.root, "scratch");
  const build = () => run("swift", ["build", "--scratch-path", scratch, "-v"], ws.sample);
  const outputs = () =>
    join(scratch, "plugins/outputs/packagesample/Screens/destination/WeftBuildToolPlugin");
  const generated = (file: string) => join(outputs(), "Sources/Screens", file);
  const tokens = () => join(outputs(), "WeftTokens.swift");
  afterAll(ws.cleanup);

  test("builds the target with the generated screens", { timeout: 600_000 }, () => {
    const first = build();
    expect(first.output).toContain(GENERATING);
    expect(first.output).toContain(GENERATING_TOKENS);
    expect(first.status, first.output).toBe(0);
    const login = readFileSync(generated("login.swift"), "utf8");
    expect(login).toContain("struct LoginScreen: View");
    // The screens read the shared tokens instead of carrying their own.
    expect(login).toContain("var theme = WeftTokens()");
    const review = readFileSync(generated("review.swift"), "utf8");
    expect(review).toContain("RatingView(value: Double(model.score), color: theme.color.accent)");
    // The project's token layer reached the generator through the `--project` the plugin passes.
    const shared = readFileSync(tokens(), "utf8");
    expect(shared).toContain("var md: CGFloat = 16");
    expect(shared).toContain("var accent: Color = Color(.sRGB, red: 0.85, green: 0.47, blue: 0.02");
    // One tokens file for the target, whatever the number of screens.
    expect(walk(outputs()).filter((f) => f.endsWith("/WeftTokens.swift"))).toEqual([tokens()]);
  });

  test.each([
    ["iphonesimulator", "arm64-apple-ios17.0-simulator"],
    ["macosx", "arm64-apple-macos14.0"],
  ])("the generated files typecheck for %s", { timeout: 300_000 }, (sdk, target) => {
    expect(existsSync(tokens())).toBe(true);
    const result = run("xcrun", [
      "--sdk",
      sdk,
      "swiftc",
      "-typecheck",
      "-swift-version",
      "6",
      "-target",
      target,
      generated("login.swift"),
      generated("review.swift"),
      tokens(),
      join(ws.sample, "Sources/Screens/RatingView.swift"),
    ]);
    expect(result.status, result.output).toBe(0);
  });

  test("an unchanged build runs nothing", { timeout: 600_000 }, () => {
    const again = build();
    expect(again.status, again.output).toBe(0);
    expect(again.output).not.toContain(GENERATING);
    expect(again.output).not.toContain(GENERATING_TOKENS);
  });

  test("a token file named by weft.json is an input", { timeout: 600_000 }, () => {
    const file = join(ws.sample, "tokens/brand.tokens.json");
    writeFileSync(file, readFileSync(file, "utf8").replace('"value": 16', '"value": 24'));
    const changed = build();
    expect(changed.output).toContain(GENERATING_TOKENS);
    expect(changed.status, changed.output).toBe(0);
    expect(readFileSync(tokens(), "utf8")).toContain("var md: CGFloat = 24");
  });

  test("a missing custom view fails the build and names it", { timeout: 600_000 }, () => {
    const view = join(ws.sample, "Sources/Screens/RatingView.swift");
    const source = readFileSync(view, "utf8");
    rmSync(view);
    const broken = build();
    writeFileSync(view, source);
    expect(broken.status).not.toBe(0);
    expect(broken.output).toContain("cannot find 'RatingView' in scope");
    expect(readFileSync(generated("review.swift"), "utf8")).toContain(
      "// The app writes the views for its own kinds: `RatingView` (`rating`).",
    );
  });

  test("two projects with shared tokens in one target fail clearly", { timeout: 600_000 }, () => {
    const other = join(ws.sample, "Sources/Screens/Other");
    mkdirSync(other);
    writeFileSync(join(other, "weft.json"), "{}\n");
    writeFileSync(
      join(other, "about.weft"),
      '<screen id="about" label="About" weft="0.1">\n  <text id="body">About</text>\n</screen>\n',
    );
    const broken = build();
    rmSync(other, { recursive: true });
    expect(broken.status).not.toBe(0);
    expect(broken.output).toContain("use 2 projects with shared tokens");
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
