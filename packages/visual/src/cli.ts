// The static page, the convention importers and the SwiftUI generator exist only in Rust, so the
// visual tests reach them through the `weft` CLI the Rust workspace builds (`moon run root:cli`).
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
export const CORPUS = join(ROOT, "corpus");
/** The SwiftPM sample of the Xcode plugin: a project with shared tokens and a custom component. */
export const SAMPLE = join(ROOT, "plugins/xcode/Examples/PackageSample");
/** The example project: a resolver with a light and a dark theme. */
export const EXAMPLE = join(ROOT, "examples/project");
// Built by `moon run root:cli` into a target directory of its own (see moon.yml).
const CLI = join(ROOT, "target/visual/debug/weft");

function binary(): string {
  if (!existsSync(CLI)) {
    throw new Error(`${CLI} is missing; build it with \`moon run root:cli\``);
  }
  return CLI;
}

/** Runs the Rust CLI with `args` as they are, on files that stay where their project is. */
export function weft(...args: string[]): string {
  return execFileSync(binary(), args, { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
}

/** Runs the Rust CLI on `input` written to a scratch file named `file`. */
export function cli(command: string, file: string, input: string, ...flags: string[]): string {
  binary();
  const dir = mkdtempSync(join(tmpdir(), "weft-visual-"));
  try {
    const path = join(dir, file);
    writeFileSync(path, input);
    return execFileSync(CLI, [command, "--no-project", ...flags, path], {
      encoding: "utf8",
      // Losses go to stderr; the visual comparison is what judges them here.
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch (error) {
    // An importer that writes a document failing validation still prints it and exits non-zero;
    // the document is what gets rendered, so the comparison shows what the failure costs.
    const stdout = (error as { stdout?: unknown }).stdout;
    if (typeof stdout === "string" && stdout !== "") return stdout;
    throw error;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
