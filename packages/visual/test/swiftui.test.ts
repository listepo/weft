// Every corpus screen the SwiftUI generator accepts, built into one iOS app and screenshotted in
// the iOS Simulator, then compared with its reviewed baseline. A screen changed by one point of
// layout or one word must fail the same comparison.
//
// Each screen is generated with its corpus data (`weft swiftui --data`) and shows the model's
// `sample`, the data the React screenshots show, so the screenshot pins the layout, controls and
// text with real content in them. One more screen comes from the Xcode plugin's sample project:
// it reads the project's shared `WeftTokens` and calls `RatingView`, the view the app writes for
// its own `rating` kind. The example project's review screen, whose resolver has a light and a
// dark theme, is shot in both appearances: its star colour follows the system (T45).
//
// The simulator is pinned (device and runtime below), since a baseline is only meaningful on the
// runtime that drew it. Without macOS, Xcode or that simulator the suite is skipped and says why.
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { matchBaseline } from "../src/baseline.ts";
import { CORPUS, EXAMPLE, SAMPLE, cli, weft } from "../src/cli.ts";
import { compare } from "../src/compare.ts";

const DEVICE_TYPE = "com.apple.CoreSimulator.SimDeviceType.iPhone-17";
const RUNTIME = "com.apple.CoreSimulator.SimRuntime.iOS-27-0";
const BUNDLE = "dev.weft.visual";
const TARGET = "arm64-apple-ios17.0-simulator";

// The generator refuses these screens on purpose (array-index paths); its own tests pin why.
const REFUSED = new Set(["leaderboard"]);
// The sample project's screen, named apart from the corpus screens it is listed with.
const SAMPLE_SCREEN = "sample-review";
// The example project's review screen, with its own theme, in either appearance.
const EXAMPLE_SCREEN = "example-review";
const EXAMPLE_DARK = "example-review-dark";

const run = (cmd: string, args: string[]) =>
  spawnSync(cmd, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

/** The pinned simulator's UDID, or why the suite cannot run here. */
function simulator(): { udid: string } | { reason: string } {
  if (process.platform !== "darwin") return { reason: "the iOS Simulator needs macOS" };
  if (run("xcrun", ["--find", "simctl"]).status !== 0) return { reason: "Xcode is not installed" };
  const list = run("xcrun", ["simctl", "list", "devices", "available", "-j"]);
  if (list.status !== 0) return { reason: `simctl failed: ${list.stderr}` };
  const devices = (
    JSON.parse(list.stdout) as {
      devices: Record<string, { udid: string; deviceTypeIdentifier: string }[]>;
    }
  ).devices[RUNTIME];
  const device = devices?.find((d) => d.deviceTypeIdentifier === DEVICE_TYPE);
  if (device === undefined) {
    return {
      reason: `no ${DEVICE_TYPE} simulator on ${RUNTIME}; install the runtime in Xcode and create one with \`xcrun simctl create weft ${DEVICE_TYPE} ${RUNTIME}\``,
    };
  }
  return { udid: device.udid };
}

const found = simulator();

const pascal = (name: string) =>
  name.replace(/(^|-)([a-z0-9])/g, (_, _dash: string, c: string) => c.toUpperCase());

/**
 * Renames the screen's top-level types after `type`, since two corpus screens may share a screen
 * id and every screen goes into one module.
 */
function renamed(swift: string, type: string): string {
  const prefix = /struct (\w+)Screen: View/.exec(swift)?.[1];
  if (prefix === undefined) throw new Error("no screen view in the generated Swift");
  return swift.replace(
    new RegExp(`\\b${prefix}(Model|Action|Event|Theme|Screen)\\b`, "g"),
    `${type}$1`,
  );
}

/** The deliberately changed copies of one screen the mutation tests compare. */
const MUTANTS = {
  // The smallest layout change SwiftUI source states: one point more between the login fields.
  "login-gap": (swift: string) =>
    replaceOnce(swift, "var md: CGFloat = 16", "var md: CGFloat = 17"),
  // One word for another of the same length.
  "login-word": (swift: string) => replaceOnce(swift, '"Forgot password?"', '"Forgot passcode?"'),
};

function replaceOnce(text: string, from: string, to: string): string {
  if (!text.includes(from)) throw new Error(`nothing to change: ${from}`);
  return text.replace(from, to);
}

const APP = `import SwiftUI

@main
struct WeftScreensApp: App {
    var body: some Scene {
        WindowGroup {
            WeftScreens.view(ProcessInfo.processInfo.environment["WEFT_SCREEN"] ?? "")
                // Whatever the simulator is set to, every screenshot is drawn the same way: light
                // unless the test asks for dark.
                .preferredColorScheme(
                    ProcessInfo.processInfo.environment["WEFT_SCHEME"] == "dark" ? .dark : .light
                )
                .dynamicTypeSize(.large)
                .environment(\\.locale, Locale(identifier: "en_US"))
                .statusBarHidden(true)
        }
    }
}
`;

function dispatcher(types: Map<string, string>): string {
  const cases = [...types].map(
    ([name, type]) => `        case "${name}": ${type}Screen(model: .sample)`,
  );
  return `import SwiftUI

enum WeftScreens {
    @MainActor @ViewBuilder
    static func view(_ name: String) -> some View {
        switch name {
${cases.join("\n")}
        default: Text("unknown screen \\(name)")
        }
    }
}
`;
}

const INFO = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>${BUNDLE}</string>
  <key>CFBundleExecutable</key><string>WeftScreens</string>
  <key>CFBundleName</key><string>WeftScreens</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>MinimumOSVersion</key><string>17.0</string>
  <key>UIDeviceFamily</key><array><integer>1</integer></array>
  <key>UILaunchScreen</key><dict/>
</dict>
</plist>
`;

/**
 * The sample project's review screen, with its data, the shared tokens it reads and the custom
 * view it calls. The project's files stay where they are, so the CLI finds the catalog and tokens.
 */
function sampleSources(dir: string): { screen: string; support: Map<string, string> } {
  const data = join(dir, "review.data.json");
  writeFileSync(data, JSON.stringify({ score: 4 }));
  const project = join(SAMPLE, "weft.json");
  const screen = weft(
    "swiftui",
    join(SAMPLE, "Sources/Screens/review.weft"),
    "--project",
    project,
    "--data",
    data,
  );
  const support = new Map([
    ["WeftTokens.swift", weft("swiftui-tokens", "--project", project)],
    ["RatingView.swift", readFileSync(join(SAMPLE, "Sources/Screens/RatingView.swift"), "utf8")],
  ]);
  return { screen, support };
}

/**
 * The example project's review screen with its sample data and a theme of its own, whose colours
 * that differ in the dark theme follow the appearance. It calls the same `RatingView`.
 */
function exampleSource(): string {
  return weft(
    "swiftui",
    join(EXAMPLE, "screens/review.weft"),
    "--project",
    join(EXAMPLE, "weft.json"),
    "--no-shared-tokens",
    "--data",
    join(EXAMPLE, "sample.data.json"),
  );
}

/** Writes and compiles the app with every screen in it; returns the `.app` path. */
function buildApp(dir: string, sources: Map<string, string>, support: Map<string, string>): string {
  const swift = join(dir, "src");
  mkdirSync(swift);
  const types = new Map<string, string>();
  for (const [name, source] of sources) {
    const type = pascal(name);
    types.set(name, type);
    writeFileSync(join(swift, `${type}.swift`), renamed(source, type));
  }
  for (const [file, source] of support) writeFileSync(join(swift, file), source);
  writeFileSync(join(swift, "App.swift"), APP);
  writeFileSync(join(swift, "WeftScreens.swift"), dispatcher(types));
  const app = join(dir, "WeftScreens.app");
  mkdirSync(app);
  writeFileSync(join(app, "Info.plist"), INFO);
  const files = readdirSync(swift).map((f) => join(swift, f));
  const compiled = run("xcrun", [
    "--sdk",
    "iphonesimulator",
    "swiftc",
    "-target",
    TARGET,
    "-parse-as-library",
    "-module-name",
    "WeftScreens",
    "-o",
    join(app, "WeftScreens"),
    ...files,
  ]);
  if (compiled.status !== 0) throw new Error(`swiftc failed:\n${compiled.stderr}`);
  // The simulator on Apple silicon only runs signed code; an ad-hoc signature is enough.
  execFileSync("codesign", ["--force", "--sign", "-", app], { stdio: "ignore" });
  return app;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe.skipIf("reason" in found)("SwiftUI in the iOS Simulator", () => {
  const udid = "udid" in found ? found.udid : "";
  const names = [
    ...readdirSync(CORPUS, { withFileTypes: true })
      .filter((e) => e.isDirectory() && !REFUSED.has(e.name))
      .map((e) => e.name)
      .sort(),
    SAMPLE_SCREEN,
    EXAMPLE_SCREEN,
    EXAMPLE_DARK,
  ];
  let dir = "";
  let booted = false;

  beforeAll(() => {
    dir = mkdtempSync(join(tmpdir(), "weft-swiftui-"));
    const sources = new Map<string, string>();
    const own = new Set([SAMPLE_SCREEN, EXAMPLE_SCREEN, EXAMPLE_DARK]);
    for (const name of names.filter((n) => !own.has(n))) {
      sources.set(
        name,
        cli(
          "swiftui",
          "screen.weft",
          readFileSync(join(CORPUS, name, "screen.weft"), "utf8"),
          "--data",
          join(CORPUS, name, "data.json"),
        ),
      );
    }
    const sample = sampleSources(dir);
    sources.set(SAMPLE_SCREEN, sample.screen);
    sources.set(EXAMPLE_SCREEN, exampleSource());
    const login = sources.get("login")!;
    for (const [name, mutate] of Object.entries(MUTANTS)) sources.set(name, mutate(login));
    const app = buildApp(dir, sources, sample.support);

    if (deviceState(udid) !== "Booted") {
      execFileSync("xcrun", ["simctl", "boot", udid]);
      booted = true;
    }
    execFileSync("xcrun", ["simctl", "bootstatus", udid, "-b"], { stdio: "ignore" });
    execFileSync("xcrun", ["simctl", "install", udid, app]);
  });

  afterAll(() => {
    if (udid !== "") {
      run("xcrun", ["simctl", "terminate", udid, BUNDLE]);
      run("xcrun", ["simctl", "uninstall", udid, BUNDLE]);
      // Leaves the simulator as it was found.
      if (booted) run("xcrun", ["simctl", "shutdown", udid]);
    }
    if (dir !== "") rmSync(dir, { recursive: true, force: true });
  });

  /** Launches `name` and screenshots it once two screenshots in a row agree. */
  async function shoot(name: string): Promise<Uint8Array> {
    // The dark shot is the same screen in the other appearance, not another build of it.
    const [screen, scheme] = name === EXAMPLE_DARK ? [EXAMPLE_SCREEN, "dark"] : [name, "light"];
    execFileSync("xcrun", ["simctl", "launch", "--terminate-running-process", udid, BUNDLE], {
      env: { ...process.env, SIMCTL_CHILD_WEFT_SCREEN: screen, SIMCTL_CHILD_WEFT_SCHEME: scheme },
      stdio: "ignore",
    });
    const path = join(dir, `${name}.png`);
    let previous: Buffer | undefined;
    let failure = "";
    for (let attempt = 0; attempt < 40; attempt++) {
      await sleep(attempt === 0 ? 1500 : 500);
      // A simulator that has only just booted refuses screenshots for a while; that is a wait,
      // not a failure.
      const taken = run("xcrun", ["simctl", "io", udid, "screenshot", "--type=png", path]);
      if (taken.status !== 0) {
        failure = taken.stderr.trim();
        previous = undefined;
        continue;
      }
      const png = readFileSync(path);
      if (previous?.equals(png)) return new Uint8Array(png);
      previous = png;
    }
    if (failure !== "") throw new Error(`no screenshot of ${name}: ${failure}`);
    throw new Error(`${name} never stopped changing on screen`);
  }

  const shots = new Map<string, Uint8Array>();
  const shot = async (name: string) => {
    if (!shots.has(name)) shots.set(name, await shoot(name));
    return shots.get(name)!;
  };

  for (const name of names) {
    test(`${name} matches its reviewed baseline`, async ({ skip }) => {
      const result = matchBaseline(await shot(name), `swiftui/${name}`);
      if (result.status === "missing") {
        if (result.reviewed)
          throw new Error(
            `no baseline swiftui/${name}: take it with WEFT_UPDATE_SCREENSHOTS=1 and review it`,
          );
        skip(`no reviewed baselines for ${result.platform}`);
      }
      if (result.status === "differ") {
        const { differing, diff } = result.comparison;
        throw new Error(
          `swiftui/${name}: ${differing} pixels differ from the baseline; see ${diff}`,
        );
      }
    });
  }

  test("the example screen follows the appearance", async () => {
    const light = await shot(EXAMPLE_SCREEN);
    const dark = await shot(EXAMPLE_DARK);
    expect(compare(light, dark, "appearance/swiftui-example-review").differing).toBeGreaterThan(0);
  });

  for (const mutant of Object.keys(MUTANTS)) {
    test(`${mutant} is caught against login and its baseline`, async () => {
      const changed = await shot(mutant);
      expect(
        compare(await shot("login"), changed, `mutation/swiftui-${mutant}`).differing,
      ).toBeGreaterThan(0);
      const result = matchBaseline(changed, "swiftui/login", {
        readOnly: true,
        label: `mutation/swiftui-${mutant}.baseline`,
      });
      if (result.status !== "missing") expect(result.status).toBe("differ");
    });
  }
});

/** The simulator's state, e.g. `Booted` or `Shutdown`. */
function deviceState(udid: string): string {
  const json = run("xcrun", ["simctl", "list", "devices", "-j"]).stdout;
  const all = (JSON.parse(json) as { devices: Record<string, { udid: string; state: string }[]> })
    .devices;
  for (const devices of Object.values(all)) {
    const device = devices.find((d) => d.udid === udid);
    if (device !== undefined) return device.state;
  }
  return "";
}

if ("reason" in found) {
  console.warn(`SwiftUI screenshots skipped: ${found.reason}`);
  test.skip(`SwiftUI screenshots: ${found.reason}`, () => {});
}
