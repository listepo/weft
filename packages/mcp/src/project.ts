import { loadProject, tokenTypes } from "@weft/catalog";
import { hasErrors, type Diagnostic, type Mode } from "@weft/core";
import { z } from "zod";
import { LIMITS, type Context } from "./context.ts";

// Unknown rather than an object schema: a malformed project then gets W701 diagnostics that point
// at the member to fix, instead of a generic argument error.
export const projectSchema = z
  .unknown()
  .optional()
  .describe(
    `The project's weft.json with each file name replaced by that file's JSON content: {"tokens":[DTCG token trees, later layers win],"catalog":{catalog extension},"actions":["name",…],"data":{JSON Schema of the data model}}, every member optional. It replaces the server's catalog, tokens, actions and data schema for this call. At most ${LIMITS.projectChars} characters of JSON.`,
  );

/**
 * The context one call works in: the host's, or the one its `project` argument describes. Without
 * a context the project has errors, which are among the diagnostics; its warnings are there too,
 * so a tool can return them with its own.
 */
export function scope(
  host: Context,
  project: unknown,
  mode: Mode,
): { context?: Context | undefined; diagnostics: Diagnostic[] } | { tooLong: string } {
  if (project === undefined) return { context: host, diagnostics: [] };
  if ((JSON.stringify(project) ?? "").length > LIMITS.projectChars) {
    return { tooLong: `The project is longer than ${LIMITS.projectChars} characters of JSON.` };
  }
  const loaded = loadProject(project, { prefix: "#/project", mode });
  if (hasErrors(loaded.diagnostics)) return { diagnostics: loaded.diagnostics };
  const { catalog, tokens, actions, data } = loaded.project;
  return {
    context: { catalog, tokens: tokens && tokenTypes(tokens), actions, data },
    diagnostics: loaded.diagnostics,
  };
}
