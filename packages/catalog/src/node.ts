// The file side of SPEC §10 for Node tools: finding the project above a screen and reading the
// files it names. Kept apart from the pure loader so that browser and MCP code never pull in fs.
import { readFileSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import type { Diagnostic, Mode } from "@weft/core";
import { loadProjectText, PROJECT_FILE, type Project, type ProjectResult } from "./project.ts";

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
 * The project file a tool works with (SPEC §10.1): the one named on the command line wins, a
 * refusal (`--no-project`) means none, and otherwise the one found above the input.
 */
export function chooseProject(
  input: string,
  options: { project?: string | undefined; noProject?: boolean | undefined },
): string | undefined {
  if (options.noProject === true) return undefined;
  return options.project ?? findProject(input);
}

/** A file name from the project (or one of its settings) as a path: names are relative to it. */
export function projectPath(projectFile: string, name: string): string {
  return join(dirname(resolve(projectFile)), name);
}

export type ReadOptions = {
  mode?: Mode | undefined;
  /** Files longer than this many characters count as unreadable; the project file itself throws. */
  maxChars?: number | undefined;
};

/**
 * Loads a project file. A project file that cannot be read throws, as any missing input file
 * does; everything inside it is reported as diagnostics.
 */
export function readProject(file: string, options: ReadOptions = {}): ProjectResult {
  const text = readBounded(file, options.maxChars);
  return loadProjectText(text, { mode: options.mode, read: reader(file, options.maxChars) });
}

/**
 * Token files named by a setting such as `render.tokens` (or one resolver document), layered as the project's own `tokens`
 * are (SPEC §10.3). The loader does the layering, so its diagnostics point at `pointer/tokens/…`.
 */
export function readTokenLayers(
  projectFile: string,
  names: readonly string[] | string,
  pointer: string,
  options: ReadOptions = {},
): Pick<Project, "tokens" | "modifiers" | "appearance"> & { diagnostics: Diagnostic[] } {
  const { project, diagnostics } = loadProjectText(JSON.stringify({ tokens: names }), {
    mode: options.mode,
    prefix: pointer,
    read: reader(projectFile, options.maxChars),
  });
  const { tokens, modifiers, appearance } = project;
  return { tokens, modifiers, appearance, diagnostics };
}

/**
 * A DTCG resolver named on the command line (`--tokens`): it loads as the `tokens` of a project
 * beside it, so its references follow the same rules (SPEC §10.3), and its diagnostics point into
 * the resolver file itself.
 */
export function readResolver(
  file: string,
  options: ReadOptions = {},
): ReturnType<typeof readTokenLayers> {
  const layers = readTokenLayers(file, basename(file), "#", options);
  const diagnostics = layers.diagnostics.map((d) => ({
    ...d,
    path: d.path.replace(/^#\/tokens(?=\/|$)/, "#"),
  }));
  return { ...layers, diagnostics };
}

function reader(projectFile: string, maxChars: number | undefined) {
  // The loader has already refused names that leave the directory.
  return (name: string): string | undefined => {
    try {
      return readBounded(projectPath(projectFile, name), maxChars);
    } catch {
      return undefined;
    }
  };
}

function readBounded(file: string, maxChars: number | undefined): string {
  // A UTF-8 character is at most 4 bytes, so a larger file is refused before it is read.
  if (maxChars !== undefined && statSync(file).size > maxChars * 4) {
    throw new Error(`${file} is longer than ${maxChars} characters`);
  }
  const text = readFileSync(file, "utf8");
  if (maxChars !== undefined && text.length > maxChars) {
    throw new Error(`${file} is longer than ${maxChars} characters`);
  }
  return text;
}
