import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { hasErrors } from "@weft/core";
import { z } from "zod";
import { LIMITS, type Context } from "../context.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, guarded, text } from "../result.ts";

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
        'Checks Weft markup against the catalog: syntax, schema and semantics. Returns {"valid":boolean,"diagnostics":[…]}. Each diagnostic has a stable code, severity, the path of the element, what was expected and, when known, a hint with the smallest fix. Warnings (severity "warning") do not make the markup invalid. Set strict to also reject unknown elements and attributes, as a writer must.',
      inputSchema: {
        markup: markupSchema,
        strict: z
          .boolean()
          .optional()
          .describe("Treat unknown elements and attributes as errors. Default false."),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, strict }) => {
      const { diagnostics } = readMarkup(markup, context, strict === true ? "strict" : "lenient");
      const valid = !hasErrors(diagnostics);
      return text(`{"valid":${valid},${diagnosticsText(diagnostics).slice(1)}`);
    }),
  );
}
