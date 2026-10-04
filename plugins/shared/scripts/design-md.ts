#!/usr/bin/env node
// A design system → DTCG tokens for the project, with the loss table of what did not carry over:
//   node design-md.ts <DESIGN.md | tokens.css | folder> [out.tokens.json] [--force] [--project weft.json | --no-project]
// A DESIGN.md is read for its YAML frontmatter (Google Labs format), a tokens.css for its :root custom
// properties, and a folder (an Open Design design system) for its tokens.css, else its DESIGN.md.
// `plugins.open-design.tokensDir` (SPEC §10.6) in the project says where the file goes.
import { statSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { parseArgs } from "node:util";
import { fromDesignMd, fromTokensCss, MAX_SOURCE_LENGTH, type Mapped } from "@weft/design-md";
import {
  defaultIo,
  EXIT,
  lossTable,
  openProject,
  PROJECT_OPTIONS,
  readText,
  targetPath,
  writeOutput,
  type Io,
  type Workspace,
} from "./lib.ts";

const USAGE =
  "usage: design-md <DESIGN.md | tokens.css | folder> [out.tokens.json] [--force] [--project weft.json | --no-project]\n";

/** The source file a design system folder or file stands for, and how to read it. */
function pickSource(
  input: string,
  io: Io,
): { file: string; read: (text: string) => Mapped } | undefined {
  let isDirectory: boolean;
  try {
    isDirectory = statSync(input).isDirectory();
  } catch (error) {
    io.stderr(`weft: cannot read ${input}: ${(error as Error).message}\n`);
    return undefined;
  }
  const byName = (file: string) =>
    file.toLowerCase().endsWith(".css") ? fromTokensCss : fromDesignMd;
  if (!isDirectory) return { file: input, read: byName(input) };
  for (const name of ["tokens.css", "DESIGN.md"]) {
    const file = join(input, name);
    try {
      if (statSync(file).isFile()) return { file, read: byName(file) };
    } catch {
      // The next candidate may exist.
    }
  }
  io.stderr(`weft: ${input} holds neither tokens.css nor DESIGN.md\n`);
  return undefined;
}

/**
 * The plugin's own setting. The project loader checks only that `plugins.<name>` is an object, so the
 * value is checked here, and a directory that leaves the project is refused as the loader would.
 */
function tokensDir(workspace: Workspace, io: Io): { dir?: string | undefined } | number {
  const value = workspace.project?.settings.plugins?.["open-design"]?.["tokensDir"];
  if (value === undefined || workspace.file === undefined) return {};
  // A path is joined onto the project folder, so an absolute one would silently become relative to it.
  const outside = (name: string) => isAbsolute(name) || name.split(/[\\/]/).includes("..");
  if (typeof value !== "string" || value === "" || outside(value)) {
    io.stderr(
      `weft: plugins.open-design.tokensDir in ${workspace.file} must name a folder inside the project\n`,
    );
    return EXIT.invalid;
  }
  return { dir: value };
}

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: { force: { type: "boolean" }, ...PROJECT_OPTIONS },
    });
  } catch (error) {
    io.stderr(`${(error as Error).message}\n${USAGE}`);
    return EXIT.failure;
  }
  const [input, output, ...rest] = parsed.positionals;
  if (input === undefined || rest.length > 0) {
    io.stderr(USAGE);
    return EXIT.failure;
  }
  const source = pickSource(input, io);
  if (source === undefined) return EXIT.failure;
  const text = readText(source.file, MAX_SOURCE_LENGTH, io);
  if (text === undefined) return EXIT.failure;
  const workspace = openProject(input, parsed.values, io);
  if (typeof workspace === "number") return workspace;
  const setting = tokensDir(workspace, io);
  if (typeof setting === "number") return setting;

  const { tokens, losses, problems } = source.read(text);
  for (const problem of problems) io.stderr(`weft: ${source.file}: ${problem}\n`);
  if (Object.keys(tokens).length === 0) {
    io.stderr(`weft: no tokens were found in ${source.file}\n`);
    return EXIT.invalid;
  }
  const target = targetPath(input, ".tokens.json", output, workspace, setting.dir, io);
  if (target === undefined) return EXIT.failure;
  if (
    !writeOutput(target, `${JSON.stringify(tokens, null, 2)}\n`, parsed.values.force === true, io)
  ) {
    return EXIT.failure;
  }
  const listed =
    workspace.file === undefined
      ? ""
      : `\nList it under "tokens" in ${workspace.file} to use it.\n`;
  io.stdout(`Wrote ${target}\n${listed}\nLosses:\n\n${lossTable(losses)}`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
