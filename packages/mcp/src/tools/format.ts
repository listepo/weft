import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { serialize } from "@weft/core";
import { LIMITS, type Context } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";
import { markupSchema } from "./validate.ts";

export function registerFormat(server: McpServer, context: Context): void {
  server.registerTool(
    "weft_format",
    {
      title: "Format Weft markup",
      description: `Rewrites valid Weft markup into its canonical form: attributes and slots in a fixed order, two-space indentation, whitespace normalized, comments dropped. Two documents that mean the same have the same canonical text. Returns the canonical markup. If the markup has errors it returns the diagnostics instead and marks the result as an error; fix them and call again. Input is limited to ${LIMITS.markupChars} characters.`,
      inputSchema: { markup: markupSchema, project: projectSchema },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, project }) => {
      const scoped = scope(context, project, "lenient");
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      if (scoped.context === undefined) return failure(diagnosticsText(scoped.diagnostics));
      const { document, diagnostics, ok } = readMarkup(markup, scoped.context, "lenient");
      const all = [...scoped.diagnostics, ...diagnostics];
      if (!ok || document === undefined) return failure(diagnosticsText(all));
      return warned(serialize(document), all);
    }),
  );
}

/** Markup first; warnings, if any, in a second block so the markup can be used as is. */
export function warned(markup: string, diagnostics: Parameters<typeof diagnosticsText>[0]) {
  const result = text(markup);
  if (diagnostics.length > 0)
    result.content.push({ type: "text", text: diagnosticsText(diagnostics) });
  return result;
}
