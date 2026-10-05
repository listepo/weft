// What the Xcode plugin tests share: finding Xcode, running its tools, and a scratch copy of the
// package and a sample that tests may change without touching the repository.
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  symlinkSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const pluginDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const repoRoot = resolve(pluginDir, "../..");
export const bundle = join(pluginDir, "Artifacts/weft.artifactbundle");

export interface Result {
  status: number | null;
  output: string;
}

export const run = (command: string, args: string[], cwd?: string, input?: string): Result => {
  const result = spawnSync(command, args, { cwd, input, encoding: "utf8", maxBuffer: 1 << 28 });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
};

const works = (command: string, args: string[]): boolean => {
  try {
    return spawnSync(command, args, { stdio: "ignore" }).status === 0;
  } catch {
    return false;
  }
};

// Everything here needs macOS with Xcode and the artifact bundle that `xcode-plugin:bundle`
// builds. Elsewhere the suites skip and say why, as the Swift typecheck tests of weft-swiftui do.
export const skipReason = (() => {
  if (process.platform !== "darwin") return "not macOS";
  if (!works("xcrun", ["--find", "swiftc"])) return "xcrun swiftc is not available (no Xcode)";
  if (!works("xcrun", ["--find", "xcodebuild"])) return "xcodebuild is not available (no Xcode)";
  if (!existsSync(bundle)) return "no artifact bundle (run `moon run xcode-plugin:bundle`)";
  return undefined;
})();
if (skipReason) {
  console.warn(`skipped: the Xcode plugin tests need macOS with Xcode: ${skipReason}`);
}

export const hasXcodegen = works("xcodegen", ["--version"]);
if (!hasXcodegen) {
  console.warn("skipped: xcodegen is not installed, so the Xcode project samples are not built");
}

/** The `weft` executable inside the bundle, the same one the plugins run. */
export const weftBinary = (): string => {
  const variant = readdirSync(bundle).find((name) => name.startsWith("weft-"));
  if (!variant) throw new Error(`no variant in ${bundle}`);
  return join(bundle, variant, "bin/weft");
};

export const tempDir = (name: string): string => mkdtempSync(join(tmpdir(), `weft-xcode-${name}-`));

export const remove = (dir: string) => rmSync(dir, { recursive: true, force: true });

/**
 * A scratch workspace shaped like the repository: the package's sources and bundle linked in at
 * `xcode/`, and a copy of the sample at `xcode/Examples/<name>` (links in the sample are
 * replaced by their files, because the copy no longer sits next to `corpus/`). The samples name
 * the package by the relative path `../..`, so the layout must stay.
 */
export const workspace = (sample: string): { root: string; sample: string; cleanup: () => void } => {
  const root = tempDir(sample);
  const pkg = join(root, "xcode");
  mkdirSync(join(pkg, "Examples"), { recursive: true });
  for (const entry of ["Package.swift", "Plugins", "Shared", "Artifacts"]) {
    symlinkSync(join(pluginDir, entry), join(pkg, entry));
  }
  const copy = join(pkg, "Examples", sample);
  cpSync(join(pluginDir, "Examples", sample), copy, {
    recursive: true,
    dereference: true,
    filter: (source) => !/\.(build|swiftpm|xcodeproj)$/.test(source),
  });
  return { root, sample: copy, cleanup: () => remove(root) };
};
