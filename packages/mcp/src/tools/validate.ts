import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { hasErrors } from "@weft/core";
import { z } from "zod";
import { LIMITS, type Context } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";

export const markupSchema = z
  .string()
  .max(LIMITS.markupChars)
  .describe(`Weft markup, at most ${LIMITS.markupChars} characters.`);

export function registerValidate(server: McpServer, context: Context): void {
  server.registerTool(
    "weft_validate",
    {
      title: "Validate Weft markup",
      description:
        'Checks Weft markup against the catalog: syntax, schema and semantics. Returns {"valid":boolean,"diagnostics":[…]}. Each diagnostic has a stable code, severity, the path of the element, what was expected and, when known, a hint with the smallest fix. Warnings (severity "warning") do not make the markup invalid. Set strict to also reject unknown elements and attributes, as a writer must. Pass project to check against the project\'s catalog, tokens, actions and data schema; its own problems come first, with paths that start at #/project.',
      inputSchema: {
        markup: markupSchema,
        strict: z
          .boolean()
          .optional()
          .describe("Treat unknown elements and attributes as errors. Default false."),
        project: projectSchema,
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, strict, project }) => {
      const mode = strict === true ? "strict" : "lenient";
      const scoped = scope(context, project, mode);
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      const diagnostics =
        scoped.context === undefined
          ? scoped.diagnostics
          : [...scoped.diagnostics, ...readMarkup(markup, scoped.context, mode).diagnostics];
      const valid = !hasErrors(diagnostics);
      return text(`{"valid":${valid},${diagnosticsText(diagnostics).slice(1)}`);
    }),
  );
}
