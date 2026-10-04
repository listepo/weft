// The file side of SPEC §10 for Node tools: finding the project above a screen and reading the
// files it names. Kept apart from the pure loader so that browser and MCP code never pull in fs.
import { readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import type { Mode } from "@weft/core";
import { loadProjectText, PROJECT_FILE, type ProjectResult } from "./project.ts";

/** SPEC §10.1: the first `weft.json` in the screen's directory or one above it. */
export function findProject(screen: string): string | undefined {
  let dir = dirname(resolve(screen));
  for (;;) {
    const candidate = join(dir, PROJECT_FILE);
    if (statSync(candidate, { throwIfNoEntry: false })?.isFile() === true) return candidate;
    const parent = dirname(dir);
    if (parent === dir) return undefined;
    dir = parent;
  }
}

/**
 * Loads a project file. A project file that cannot be read throws, as any missing input file
 * does; everything inside it is reported as diagnostics.
 */
export function readProject(file: string, options: { mode?: Mode } = {}): ProjectResult {
  const text = readFileSync(file, "utf8");
  const dir = dirname(resolve(file));
  return loadProjectText(text, {
    mode: options.mode,
    // The loader has already refused names that leave the directory.
    read: (name) => {
      try {
        return readFileSync(join(dir, name), "utf8");
      } catch {
        return undefined;
      }
    },
  });
}
