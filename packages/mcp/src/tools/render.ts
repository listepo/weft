import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { expectedTree, formatAriaSnapshot, renderPage } from "@weft/render-react";
import { z } from "zod";
import { LIMITS, type Context } from "../context.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";
import { markupSchema } from "./validate.ts";

export function registerRender(server: McpServer, context: Context): void {
  server.registerTool(
    "weft_render",
    {
      title: "Render Weft markup",
      description: `Shows what a user of assistive technology, or an agent driving a browser, would get from Weft markup, without a browser: the accessibility tree the reference renderer produces, as YAML in the style of a Playwright aria snapshot (one "- role \\"name\\" [state]" line per node, text runs as "- text: …"). Pass data to resolve bindings, each loops and empty states against a sample data model; without it bindings read as missing. Set html to true to also get the static HTML page in a second block. The markup is validated strictly first; if it has errors the result is the diagnostics, marked as an error. Markup is limited to ${LIMITS.markupChars} characters and data to ${LIMITS.dataChars} characters of JSON.`,
      inputSchema: {
        markup: markupSchema,
        data: z
          .unknown()
          .optional()
          .describe('The host data model the bindings read, as JSON, e.g. {"email":"a@b.c"}.'),
        html: z
          .boolean()
          .optional()
          .describe("Also return the rendered static HTML page. Default false."),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, data, html }) => {
      if ((JSON.stringify(data) ?? "").length > LIMITS.dataChars) {
        return failure(`The data is longer than ${LIMITS.dataChars} characters of JSON.`);
      }
      const { document, diagnostics, ok } = readMarkup(markup, context, "strict");
      if (!ok || document === undefined) return failure(diagnosticsText(diagnostics));
      const options = { catalog: context.catalog, data };
      const tree = formatAriaSnapshot(expectedTree(document, options));
      const result = text(tree === "" ? "(nothing is exposed to assistive technology)" : tree);
      if (html === true) result.content.push({ type: "text", text: renderPage(document, options) });
      return result;
    }),
  );
}
