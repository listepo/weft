import { tokenTypes, type Project } from "@weft/catalog";
import type { Context, Limits, ServerSettings } from "./context.ts";

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
