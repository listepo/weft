#!/usr/bin/env node
// Entry point: serves the Weft tools over stdio. stdout carries the protocol, so nothing else
// may write to it.
//   weft-mcp [--project weft.json]
// With a project file its catalog, tokens, actions and data schema are the server's, and its
// `validate.mode`, `mcp.limits` and `mcp.context` settings apply (SPEC §10.6). The host names the file; the
// server never looks for one, because it has no screen to look from.
import { parseArgs } from "node:util";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { readProject } from "@weft/catalog/node";
import { hasErrors } from "@weft/core";
import { LIMITS } from "./context.ts";
import { createServer } from "./create-server.ts";
import { hostOptions } from "./project.ts";

/** The server for these arguments, or the exit code after the problem has gone to stderr. */
export function serverFor(argv: readonly string[], stderr: (text: string) => void) {
  let file: string | undefined;
  try {
    file = parseArgs({ args: [...argv], options: { project: { type: "string" } } }).values.project;
  } catch (error) {
    stderr(`${(error as Error).message}\nusage: weft-mcp [--project weft.json]\n`);
    return 2;
  }
  if (file === undefined) return createServer();
  let loaded;
  try {
    loaded = readProject(file, { maxChars: LIMITS.projectChars });
  } catch (error) {
    stderr(`weft-mcp: cannot read ${file}: ${(error as Error).message}\n`);
    return 2;
  }
  for (const d of loaded.diagnostics) stderr(`${file}:${d.path} ${d.code} ${d.message}\n`);
  // Serving with half a project would judge every screen against the wrong context.
  if (hasErrors(loaded.diagnostics)) return 1;
  const { context, settings } = hostOptions(loaded.project);
  return createServer(context, settings);
}

if (import.meta.main) {
  const server = serverFor(process.argv.slice(2), (text) => process.stderr.write(text));
  if (typeof server === "number") process.exitCode = server;
  else await server.connect(new StdioServerTransport());
}
