// Generates WeftStudio.xcodeproj (not committed). XcodeGen reads slint-bindings' Swift package
// from SLINT_BINDINGS_DIR, the checkout Cargo made of the pinned rev, so the Swift views and the
// Rust library linked into the app are always the same commit.
//
//   node studio/macos/scripts/xcodegen.ts
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { repoRoot, slintBindingsDir } from "../../scripts/slint-bindings.ts";

export const macos = join(repoRoot, "studio/macos");

/** Runs XcodeGen in `studio/macos`; returns its exit status and output. */
export const generateProject = (): { status: number | null; output: string } => {
  const result = spawnSync("xcodegen", ["generate", "--quiet"], {
    cwd: macos,
    encoding: "utf8",
    env: { ...process.env, SLINT_BINDINGS_DIR: slintBindingsDir() },
  });
  return { status: result.status, output: `${result.stdout}${result.stderr}` };
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { status, output } = generateProject();
  process.stdout.write(output);
  process.exit(status ?? 1);
}
