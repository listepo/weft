#!/usr/bin/env node
// A frame of a Figma file → `.weft` screen, read through the Figma REST API (`weft figma pull`):
//   node figma-pull.ts <figma link | file key> [out.weft] [--node <id>]
//     [--force] [--project weft.json | --no-project]
// The token is read from the environment only, never from a flag, so it stays out of shell
// history and process lists, and requests go to api.figma.com only. The project above the working
// directory (SPEC §10.1) supplies the catalog and tokens, and `import.figma` (SPEC §10.6) the
// output folder and whether the frame's context is kept.
import { join } from "node:path";
import { parseArgs } from "node:util";
import { loadTokens } from "@weft/catalog";
import { hasErrors, serialize } from "@weft/core";
import { parseTarget, pullScreen, PullError } from "@weft/figma/pull";
// A JSON module, not a path under the repository, so the bundle needs no file beside it.
import defaultTokens from "../../../packages/catalog/tokens/default.tokens.json" with { type: "json" };
import {
  contextFor,
  defaultIo,
  EXIT,
  lossTable,
  openProject,
  printDiagnostics,
  PROJECT_OPTIONS,
  targetPath,
  writeOutput,
  type Io,
} from "./lib.ts";

const USAGE =
  "usage: figma-pull <figma link | file key> [out.weft] [--node <id>] [--force] [--project weft.json | --no-project]\nThe access token is read from FIGMA_TOKEN.\n";

// A screen id becomes the file name only when it is a plain name.
const SAFE_NAME = /^[A-Za-z0-9_-]{1,100}$/;

/** What the script takes from its process; tests pass their own. */
export type Host = {
  env: Readonly<Record<string, string | undefined>>;
  fetch: typeof fetch;
  cwd: string;
};

const processHost = (): Host => ({ env: process.env, fetch, cwd: process.cwd() });

export async function main(
  argv: readonly string[],
  io: Io = defaultIo,
  host: Host = processHost(),
): Promise<number> {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: {
        node: { type: "string" },
        force: { type: "boolean" },
        ...PROJECT_OPTIONS,
      },
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
  const target = parseTarget(input, parsed.values.node);
  if ("error" in target) {
    io.stderr(`weft: ${target.error}\n${USAGE}`);
    return EXIT.failure;
  }
  const token = host.env["FIGMA_TOKEN"] ?? "";
  if (token === "") {
    io.stderr(`weft: set FIGMA_TOKEN to a Figma access token with the file_content:read scope\n`);
    return EXIT.failure;
  }
  const here = join(host.cwd, "figma");
  const workspace = openProject(here, parsed.values, io);
  if (typeof workspace === "number") return workspace;
  const { catalog } = contextFor(workspace);
  const tokens = workspace.project?.tokens ?? loadTokens(defaultTokens).tokens;
  const settings = workspace.project?.settings.import?.figma;

  let result;
  try {
    result = await pullScreen({
      ...target,
      catalog,
      tokens,
      context: settings?.context,
      token,
      fetch: host.fetch,
    });
  } catch (error) {
    if (!(error instanceof PullError)) throw error;
    io.stderr(`weft: cannot pull ${target.nodeId} from Figma: ${error.message}\n`);
    return EXIT.failure;
  }
  const { document, losses, diagnostics } = result;
  printDiagnostics(`figma:${target.nodeId}`, diagnostics, io);
  if (hasErrors(diagnostics)) return EXIT.invalid;

  const id = document.root.id;
  const name = id !== undefined && SAFE_NAME.test(id) ? id : "screen";
  const file = targetPath(join(host.cwd, name), ".weft", output, workspace, settings?.outDir, io);
  if (file === undefined) return EXIT.failure;
  if (!writeOutput(file, serialize(document), parsed.values.force === true, io))
    return EXIT.failure;
  io.stdout(`Wrote ${file}\n\nImport losses:\n\n${lossTable(losses)}`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = await main(process.argv.slice(2));
