// The static page, the convention importers and the SwiftUI generator exist only in Rust, so the
// visual tests reach them through the `weft` CLI the Rust workspace builds (`moon run root:cli`).
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../../", import.meta.url));
export const CORPUS = join(ROOT, "corpus");
// Built by `moon run root:cli` into a target directory of its own (see moon.yml).
const CLI = join(ROOT, "target/visual/debug/weft");

/** Runs the Rust CLI on `input` written to a scratch file named `file`. */
export function cli(command: string, file: string, input: string, ...flags: string[]): string {
  if (!existsSync(CLI)) {
    throw new Error(`${CLI} is missing; build it with \`moon run root:cli\``);
  }
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
