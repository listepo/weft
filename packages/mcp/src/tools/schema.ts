import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { documentSchema } from "@weft/core/document-schema";
import type { Context, ServerSettings } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";

/**
 * `weft_schema`: the JSON Schema of the canonical documents the catalog admits (SPEC §3.1), the
 * text `documentSchema` of `@weft/core/document-schema` returns, so a host can hand it to a
 * provider's structured output. A tool of its own rather than an argument of `weft_catalog`
 * (creator default for T12 question 4): the two answer different callers.
 */
export function registerSchema(
  server: McpServer,
  context: Context,
  { limits }: ServerSettings,
): void {
  server.registerTool(
    "weft_schema",
    {
      title: "Weft document JSON Schema",
      description:
        "Returns the JSON Schema (2020-12) of a Weft screen written as canonical JSON (SPEC §3) with the components of the catalog: every kind, prop, allowed value, slot and child rule it declares. It is meant for a structured-output or constrained-decoding mode that takes a JSON Schema, not for reading: it is large, and markup with weft_catalog is the usual way to write. The validator still decides; it checks ids, bindings, tokens and actions the schema cannot. Pass project to get the schema of the project's catalog: the core components with its extension merged in.",
      inputSchema: { project: projectSchema(limits) },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ project }) => {
      const scoped = scope(context, project, "lenient", limits);
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      if (scoped.context === undefined)
        return failure(diagnosticsText(scoped.diagnostics, limits.diagnostics));
      return text(documentSchema(scoped.context.catalog));
    }),
  );
}
