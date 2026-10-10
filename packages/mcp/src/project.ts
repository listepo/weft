import { loadProject, tokenTypes, type Project } from "@weft/catalog";
import { hasErrors, type Diagnostic, type Mode } from "@weft/core";
import { z } from "zod";
import { LIMITS, type Context, type Limits, type ServerSettings } from "./context.ts";

// Unknown rather than an object schema: a malformed project then gets W701 diagnostics that point
// at the member to fix, instead of a generic argument error.
export const projectSchema = (limits: Limits = LIMITS) =>
  z
    .unknown()
    .optional()
    .describe(
      `The project's weft.json with each file name replaced by that file's JSON content: {"tokens":[DTCG token trees, later layers win] or one DTCG resolver document with inline sources,"catalog":{catalog} or [{catalog},…], libraries with a "prefix" and at most one project catalog without,"actions":["name",…],"data":{JSON Schema of the data model},"fragments":{"name":"<fragment …> markup"}}, every member optional. It replaces the server's catalogs, tokens, actions and data schema for this call; its tool settings are ignored. At most ${limits.projectChars} characters of JSON.`,
    );

/** What a loaded project gives a tool to judge markup with. */
export function contextOf(project: Project): Context {
  const { catalog, catalogs, kinds, tokens, actions, data } = project;
  return { catalog, catalogs, kinds, tokens: tokens && tokenTypes(tokens), actions, data };
}

/**
 * What a project file sets for a server started with it (`weft-mcp --project`): its resources
 * and the `validate.mode`, `mcp.limits` and `mcp.context` settings (SPEC §10.6).
 */
export function hostOptions(project: Project): {
  context: Context;
  settings: {
    mode?: ServerSettings["mode"] | undefined;
    limits?: Partial<Limits> | undefined;
    context?: ServerSettings["context"] | undefined;
  };
} {
  return {
    context: contextOf(project),
    settings: {
      mode: project.settings.validate?.mode,
      limits: project.settings.mcp?.limits,
      context: project.settings.mcp?.context,
    },
  };
}

/**
 * The context one call works in: the host's, or the one its `project` argument describes. Without
 * a context the project has errors, which are among the diagnostics; its warnings are there too,
 * so a tool can return them with its own.
 */
export function scope(
  host: Context,
  project: unknown,
  mode: Mode,
  limits: Limits,
): { context?: Context | undefined; diagnostics: Diagnostic[] } | { tooLong: string } {
  if (project === undefined) return { context: host, diagnostics: [] };
  if ((JSON.stringify(project) ?? "").length > limits.projectChars) {
    return { tooLong: `The project is longer than ${limits.projectChars} characters of JSON.` };
  }
  const loaded = loadProject(project, { prefix: "#/project", mode });
  if (hasErrors(loaded.diagnostics)) return { diagnostics: loaded.diagnostics };
  return { context: contextOf(loaded.project), diagnostics: loaded.diagnostics };
}
