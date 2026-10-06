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
//
// Runs started at once in several worktrees would overwrite each other's app and screenshots on one
// device, so `WEFT_SIMULATOR` chooses (see ../src/simulator.ts): `shared`, the default, takes a
// machine-wide lock for the whole suite so runs take turns on one device; `own` gives each
// worktree a device of its own, created on first use.
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { matchBaseline } from "../src/baseline.ts";
import { CORPUS, EXAMPLE, ROOT, SAMPLE, cli, weft } from "../src/cli.ts";
import { compare } from "../src/compare.ts";
import {
  LOCK_DIR,
  acquireLock,
  deviceName,
  findDevice,
  simulatorMode,
  type SimulatorDevice,
} from "../src/simulator.ts";

const DEVICE_TYPE = "com.apple.CoreSimulator.SimDeviceType.iPhone-17";
const RUNTIME = "com.apple.CoreSimulator.SimRuntime.iOS-27-0";
const BUNDLE = "dev.weft.visual";
const TARGET = "arm64-apple-ios17.0-simulator";
// A queue of runs is the usual reason to wait for the lock; a run takes a few minutes on an idle
// machine and over twenty when several agents load it, so the wait must outlast one such run.
const LOCK_TIMEOUT_MS = 30 * 60_000;
// Building the app comes on top of the wait for the lock, in the same hook.
const SETUP_TIMEOUT_MS = LOCK_TIMEOUT_MS + 300_000;

// The generator refuses these screens on purpose (array-index paths); its own tests pin why.
const REFUSED = new Set(["leaderboard"]);
// The sample project's screen, named apart from the corpus screens it is listed with.
const SAMPLE_SCREEN = "sample-review";
// The example project's review screen, with its own theme, in either appearance.
const EXAMPLE_SCREEN = "example-review";
const EXAMPLE_DARK = "example-review-dark";
// The corpus glass screen in the dark appearance (T51): its glass is drawn by the system, which
// adapts to the appearance by itself.
const GLASS_DARK = "glass-dark";
// The screens with a glass material, shown over a backdrop the glass can blur.
const OVER_BACKDROP = new Set(["glass", EXAMPLE_SCREEN]);
// The system draws the glass itself and its blur is not reproducible to the pixel under load: a
// glass-dark run once differed from its baseline by 2 pixels (of 3.2 million) at the glass corners
// and passed on rerun (T60). These screens, and only these, accept up to this many differing
// pixels; any real change to a glass screen moves far more.
const GLASS_TOLERANCE = 16;
// A dark shot is its light screen again, in the other appearance.
const DARK_OF = new Map([
  [EXAMPLE_DARK, EXAMPLE_SCREEN],
  [GLASS_DARK, "glass"],
]);

const toleranceOf = (name: string) =>
  OVER_BACKDROP.has(DARK_OF.get(name) ?? name) ? GLASS_TOLERANCE : 0;

const run = (cmd: string, args: string[]) =>
  spawnSync(cmd, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

type Found = { mode: "shared"; udid: string } | { mode: "own"; name: string } | { reason: string };

type DeviceList = Record<string, SimulatorDevice[]>;

const availableDevices = (): DeviceList =>
  (
    JSON.parse(run("xcrun", ["simctl", "list", "devices", "available", "-j"]).stdout) as {
      devices: DeviceList;
    }
  ).devices;

/** Whether `simctl list <what>` names `identifier`, e.g. the runtime an own device is made from. */
function simctlKnows(what: "runtimes" | "devicetypes", identifier: string): boolean {
  const args = ["simctl", "list", what, ...(what === "runtimes" ? ["available"] : []), "-j"];
  const listed = run("xcrun", args);
  if (listed.status !== 0) return false;
  const entries = (JSON.parse(listed.stdout) as Record<string, { identifier: string }[]>)[what];
  return entries?.some((e) => e.identifier === identifier) ?? false;
}

/**
 * The simulator to use, or why the suite cannot run here. Own mode only checks that the pinned
 * device type and runtime exist; the device itself is created once the lock is held.
 */
function simulator(): Found {
  if (process.platform !== "darwin") return { reason: "the iOS Simulator needs macOS" };
  if (run("xcrun", ["--find", "simctl"]).status !== 0) return { reason: "Xcode is not installed" };
  // A mistyped setting is an error, not a skip: it would silently run in the other mode.
  const mode = simulatorMode();
  if (mode === "own") {
    if (!simctlKnows("devicetypes", DEVICE_TYPE) || !simctlKnows("runtimes", RUNTIME)) {
      return { reason: `no ${DEVICE_TYPE} on ${RUNTIME}; install the runtime in Xcode` };
    }
    return { mode, name: deviceName(basename(ROOT)) };
  }
  const list = run("xcrun", ["simctl", "list", "devices", "available", "-j"]);
  if (list.status !== 0) return { reason: `simctl failed: ${list.stderr}` };
  const device = findDevice(
    (JSON.parse(list.stdout) as { devices: DeviceList }).devices,
    RUNTIME,
    DEVICE_TYPE,
  );
  if (device === undefined) {
    return {
      reason: `no ${DEVICE_TYPE} simulator on ${RUNTIME}; install the runtime in Xcode and create one with \`xcrun simctl create weft ${DEVICE_TYPE} ${RUNTIME}\``,
    };
  }
  return { mode, udid: device.udid };
}

/** The worktree's own device, created the first time and reused afterwards. */
function ownDevice(name: string): string {
  const existing = findDevice(availableDevices(), RUNTIME, DEVICE_TYPE, name);
  if (existing !== undefined) return existing.udid;
  const created = run("xcrun", ["simctl", "create", name, DEVICE_TYPE, RUNTIME]);
  if (created.status !== 0) throw new Error(`simctl create ${name} failed: ${created.stderr}`);
  const udid = created.stdout.trim();
  console.warn(
    `Created the simulator ${name} (${udid}) for this worktree; delete it with \`xcrun simctl delete ${name}\`.`,
  );
  return udid;
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
                .background(InsetProbe().ignoresSafeArea())
        }
    }
}

// What a glass screen is shown over, as in the Chromium stage: diagonal stripes in three colours,
// darker in the dark appearance. Glass blurs what is behind it, and plain white has nothing to blur.
struct GlassBackdrop<Content: View>: View {
    @Environment(\\.colorScheme) private var scheme
    @ViewBuilder var content: Content

    var body: some View {
        content
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(Canvas { context, size in
                let colors: [Color] = scheme == .dark
                    ? [Color(red: 0.533, green: 0.075, blue: 0.216), Color(red: 0.118, green: 0.227, blue: 0.541), Color(red: 0.522, green: 0.302, blue: 0.055)]
                    : [Color(red: 0.882, green: 0.114, blue: 0.282), Color(red: 0.145, green: 0.388, blue: 0.922), Color(red: 0.98, green: 0.8, blue: 0.082)]
                let step: CGFloat = 28
                var x = -size.height
                var i = 0
                while x < size.width {
                    var stripe = Path()
                    stripe.move(to: CGPoint(x: x, y: 0))
                    stripe.addLine(to: CGPoint(x: x + step, y: 0))
                    stripe.addLine(to: CGPoint(x: x + step + size.height, y: size.height))
                    stripe.addLine(to: CGPoint(x: x + size.height, y: size.height))
                    stripe.closeSubpath()
                    context.fill(stripe, with: .color(colors[i % 3]))
                    x += step
                    i += 1
                }
            }.ignoresSafeArea())
    }
}

// The system draws the home indicator inside the bottom safe-area inset, in some screenshots and
// not in others, and an app cannot switch it off: persistentSystemOverlays(.hidden) only asks the
// system to hide it, and a run with it still showed the indicator in four screenshots. The same
// goes for the top: some screenshots include the Dynamic Island and the screen's rounded corners
// (T60). The suite therefore leaves both strips out of every comparison, and the app says how tall
// they are on this device, so no size is written down here. They are read from the window, because
// a view that ignores the safe area is told the inset is zero.
struct InsetProbe: View {
    var body: some View {
        Color.clear.task {
            while true {
                let window = UIApplication.shared.connectedScenes
                    .compactMap { ($0 as? UIWindowScene)?.keyWindow }.first
                if let window, window.bounds.width > 0 {
                    let json = "{\\"width\\": \\(window.bounds.width), \\"top\\": \\(window.safeAreaInsets.top), \\"bottom\\": \\(window.safeAreaInsets.bottom)}"
                    let dir = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
                    try? json.write(to: dir.appendingPathComponent("insets.json"), atomically: true, encoding: .utf8)
                    return
                }
                try? await Task.sleep(nanoseconds: 100_000_000)
            }
        }
    }
}
`;

function dispatcher(types: Map<string, string>): string {
  const cases = [...types].map(([name, type]) =>
    OVER_BACKDROP.has(name)
      ? `        case "${name}": GlassBackdrop { ${type}Screen(model: .sample) }`
      : `        case "${name}": ${type}Screen(model: .sample)`,
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
  let udid = "";
  let releaseLock = () => {};
  const names = [
    ...readdirSync(CORPUS, { withFileTypes: true })
      .filter((e) => e.isDirectory() && !REFUSED.has(e.name))
      .map((e) => e.name)
      .sort(),
    SAMPLE_SCREEN,
    EXAMPLE_SCREEN,
    EXAMPLE_DARK,
    GLASS_DARK,
  ];
  let dir = "";
  let booted = false;

  beforeAll(async () => {
    // Unreachable, since the suite is skipped without a simulator; narrows the type for the compiler.
    if ("reason" in found) return;
    dir = mkdtempSync(join(tmpdir(), "weft-swiftui-"));
    const sources = new Map<string, string>();
    const own = new Set([SAMPLE_SCREEN, EXAMPLE_SCREEN, EXAMPLE_DARK, GLASS_DARK]);
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

    // The lock is taken after the build, which needs no simulator, so a queue waits as little as
    // possible. An own device is private to its worktree, but two runs of that worktree still
    // share it, so it has a lock of its own.
    const lock = join(LOCK_DIR, "name" in found ? `${found.name}.lock` : "simulator.lock");
    releaseLock = await acquireLock(lock, {
      timeoutMs: LOCK_TIMEOUT_MS,
      onWait: (pid) =>
        console.warn(
          `Waiting for the iOS Simulator, which process ${pid} is using (lock ${lock}); ` +
            `WEFT_SIMULATOR=own would use a simulator of this worktree's own.`,
        ),
    });
    try {
      udid = "udid" in found ? found.udid : ownDevice(found.name);
      if (deviceState(udid) !== "Booted") {
        execFileSync("xcrun", ["simctl", "boot", udid]);
        booted = true;
      }
      execFileSync("xcrun", ["simctl", "bootstatus", udid, "-b"], { stdio: "ignore" });
      execFileSync("xcrun", ["simctl", "install", udid, app]);
    } catch (error) {
      cleanUp();
      throw error;
    }
  }, SETUP_TIMEOUT_MS);

  /** Leaves the simulator as it was found, then lets the next run in. */
  function cleanUp() {
    if (udid !== "") {
      run("xcrun", ["simctl", "terminate", udid, BUNDLE]);
      run("xcrun", ["simctl", "uninstall", udid, BUNDLE]);
      if (booted) run("xcrun", ["simctl", "shutdown", udid]);
      udid = "";
    }
    releaseLock();
  }

  afterAll(() => {
    cleanUp();
    if (dir !== "") rmSync(dir, { recursive: true, force: true });
  });

  /** Launches `name` and screenshots it once two screenshots in a row agree. */
  async function shoot(name: string): Promise<Uint8Array> {
    // The dark shot is the same screen in the other appearance, not another build of it.
    const lightScreen = DARK_OF.get(name);
    const [screen, scheme] = lightScreen !== undefined ? [lightScreen, "dark"] : [name, "light"];
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

  /**
   * The heights in screenshot pixels of the top and bottom safe-area insets, where the Dynamic
   * Island and the home indicator appear, as the launched app measured them. Read once: the
   * device does not change.
   */
  let strips: { ignoreTop: number; ignoreBottom: number } | undefined;
  async function systemStrips(png: Uint8Array) {
    if (strips !== undefined) return strips;
    const container = run("xcrun", ["simctl", "get_app_container", udid, BUNDLE, "data"]);
    if (container.status !== 0) throw new Error(`no app container: ${container.stderr.trim()}`);
    const file = join(container.stdout.trim(), "Documents", "insets.json");
    for (let attempt = 0; attempt < 40; attempt++) {
      if (existsSync(file)) {
        const { width, top, bottom } = JSON.parse(readFileSync(file, "utf8")) as {
          width: number;
          top: number;
          bottom: number;
        };
        if (!(width > 0) || !(top > 0) || !(bottom >= 0)) throw new Error(`bad insets in ${file}`);
        // A PNG's width is in its IHDR chunk, bytes 16 to 19.
        const pixels = new DataView(png.buffer, png.byteOffset).getUint32(16);
        const px = (inset: number) => Math.ceil((inset * pixels) / width);
        strips = { ignoreTop: px(top), ignoreBottom: px(bottom) };
        return strips;
      }
      await sleep(250);
    }
    throw new Error(`the app wrote no insets to ${file}`);
  }

  const shots = new Map<string, Uint8Array>();
  const shot = async (name: string) => {
    if (!shots.has(name)) shots.set(name, await shoot(name));
    return shots.get(name)!;
  };

  for (const name of names) {
    test(`${name} matches its reviewed baseline`, async ({ skip }) => {
      const png = await shot(name);
      const result = matchBaseline(png, `swiftui/${name}`, {
        ...(await systemStrips(png)),
        tolerance: toleranceOf(name),
      });
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
    const ignore = await systemStrips(light);
    expect(
      compare(light, dark, "appearance/swiftui-example-review", ignore).differing,
    ).toBeGreaterThan(0);
  });

  for (const mutant of Object.keys(MUTANTS)) {
    test(`${mutant} is caught against login and its baseline`, async () => {
      const changed = await shot(mutant);
      const ignore = await systemStrips(changed);
      expect(
        compare(await shot("login"), changed, `mutation/swiftui-${mutant}`, ignore).differing,
      ).toBeGreaterThan(0);
      const result = matchBaseline(changed, "swiftui/login", {
        readOnly: true,
        label: `mutation/swiftui-${mutant}.baseline`,
        ...ignore,
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
