// The file side of SPEC §10 for Node tools: finding the project above a screen and reading the
// files it names. Kept apart from the pure loader so that browser and MCP code never pull in fs.
import { closeSync, openSync, readFileSync, readSync, realpathSync, statSync } from "node:fs";
import { basename, dirname, join, resolve, sep } from "node:path";
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
  const root = dirname(resolve(projectFile));
  return (name: string): string | undefined => {
    try {
      const path = name.startsWith(PACKAGE_READ)
        ? packagePath(root, name)
        : confinedPath(root, name);
      return readBounded(path, maxChars);
    } catch {
      return undefined;
    }
  };
}

/** How the loader asks for a file of an npm package: `package:<npm name>/<file>` (SPEC §10.2). */
const PACKAGE_READ = "package:";
/** An npm package name, plain or scoped, as the loader accepts it. */
const PACKAGE_NAME = /^(?:@[a-z0-9~-][a-z0-9._~-]*\/)?[a-z0-9~-][a-z0-9._~-]*$/;

/**
 * SPEC §10.2 packages: the package is `node_modules/<name>` of the project directory or of the
 * nearest parent that has it, as Node resolves one, and the file must stay inside that package
 * directory (symbolic links followed). Nothing is run and nothing is fetched.
 */
function packagePath(root: string, name: string): string {
  const rest = name.slice(PACKAGE_READ.length);
  const cut = rest.startsWith("@") ? rest.indexOf("/", rest.indexOf("/") + 1) : rest.indexOf("/");
  const pkg = rest.slice(0, cut);
  if (cut < 0 || pkg.length > 214 || !PACKAGE_NAME.test(pkg)) throw new Error(`bad package ${pkg}`);
  for (let dir = root; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", pkg);
    if (statSync(join(candidate, "package.json"), { throwIfNoEntry: false })?.isFile() === true)
      return confinedPath(candidate, rest.slice(cut + 1));
    if (dirname(dir) === dir) throw new Error(`${pkg} is not installed`);
  }
}

/** SPEC §10.2: the resolved path (symbolic links followed) must stay inside the project directory. */
function confinedPath(root: string, name: string): string {
  const rootReal = realpathSync(root);
  const resolved = realpathSync(join(rootReal, name));
  const prefix = rootReal.endsWith(sep) ? rootReal : rootReal + sep;
  if (resolved !== rootReal && !resolved.startsWith(prefix)) {
    throw new Error(`${name} resolves outside the project directory`);
  }
  return resolved;
}

function readBounded(file: string, maxChars: number | undefined): string {
  if (maxChars === undefined) return readFileSync(file, "utf8");
  const fd = openSync(file, "r");
  try {
    return readCapped((buf) => readSync(fd, buf, 0, buf.length, null), maxChars, file);
  } finally {
    closeSync(fd);
  }
}

/**
 * Collects UTF-8 text through a reader that may return short reads (`0` is EOF), reading until
 * EOF or the byte ceiling (`maxChars*4+1`, so a replacement between stat and read cannot load an
 * oversized file — SPEC §10.2 untrusted input). The character ceiling is checked after decoding.
 */
export function readCapped(read: (buf: Buffer) => number, maxChars: number, label: string): string {
  const maxBytes = maxChars * 4;
  const chunkSize = 64 * 1024;
  const chunks: Buffer[] = [];
  let total = 0;
  for (;;) {
    const room = maxBytes + 1 - total;
    if (room <= 0) break;
    const buf = Buffer.alloc(Math.min(room, chunkSize));
    const n = read(buf);
    if (n === 0) break;
    total += n;
    if (total > maxBytes) {
      throw new Error(`${label} is longer than ${maxChars} characters`);
    }
    chunks.push(buf.subarray(0, n));
  }
  const text = Buffer.concat(chunks).toString("utf8");
  if (text.length > maxChars) {
    throw new Error(`${label} is longer than ${maxChars} characters`);
  }
  return text;
}
