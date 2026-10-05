#!/usr/bin/env node
// Builds `weft` in release mode for Apple Silicon and packs it as a SwiftPM artifact bundle,
// `Artifacts/weft.artifactbundle`, which Package.swift references with `binaryTarget(path:)`.
// Run with `node scripts/artifactbundle.ts [outDir]`. The bundle is a build product and is not
// committed; publishing it for `binaryTarget(url:checksum:)` waits for a GitHub remote.
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const TRIPLE = "aarch64-apple-darwin";
// The name SwiftPM uses for the same machine: it differs from Rust's.
const SWIFT_TRIPLE = "arm64-apple-macosx";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");
const outDir = resolve(process.argv[2] ?? join(here, "../Artifacts"));

// Intel macOS is not supported, and the target cannot be linked elsewhere without an SDK, so any
// other host skips with a message rather than failing a workspace-wide run.
if (process.platform !== "darwin" || process.arch !== "arm64") {
  console.log(
    `skipped: the artifact bundle is built on Apple Silicon macOS only (this host is ${process.platform}/${process.arch})`,
  );
  process.exit(0);
}

const run = (command: string, args: string[]) => {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    stdio: ["ignore", "inherit", "inherit"],
  });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status ?? result.signal}`);
  }
};

run("cargo", ["build", "--release", "--locked", "-p", "weft-cli", "--target", TRIPLE]);

const binary = join(root, "target", TRIPLE, "release", "weft");
const printed = spawnSync(binary, ["--version"], { encoding: "utf8" });
const version = printed.stdout.trim().split(" ")[1];
if (printed.status !== 0 || !version) {
  throw new Error(`cannot read the version of ${binary}`);
}

const bundle = join(outDir, "weft.artifactbundle");
const variant = `weft-${version}-macosx/bin/weft`;
rmSync(bundle, { recursive: true, force: true });
mkdirSync(dirname(join(bundle, variant)), { recursive: true });
copyFileSync(binary, join(bundle, variant));
chmodSync(join(bundle, variant), 0o755);

// The schema of SwiftPM's artifact bundle manifest (SE-0305).
const info = {
  schemaVersion: "1.0",
  artifacts: {
    weft: {
      version,
      type: "executable",
      variants: [{ path: variant, supportedTriples: [SWIFT_TRIPLE] }],
    },
  },
};
writeFileSync(join(bundle, "info.json"), `${JSON.stringify(info, null, 2)}\n`);
console.log(`wrote ${bundle} (weft ${version})`);
