import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { applyPatches, serialize } from "@weft/core";
import { z } from "zod";
import { LIMITS, type Context } from "../context.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, failure, guarded } from "../result.ts";
import { warned } from "./format.ts";
import { markupSchema } from "./validate.ts";

export function registerPatch(server: McpServer, context: Context): void {
  server.registerTool(
    "weft_patch",
    {
      title: "Patch Weft markup",
      description: `Edits existing Weft markup without rewriting it. Send the current markup and a list of patches addressed by element id; patches apply in order and all-or-nothing. On success it returns the new canonical markup; on any problem it returns diagnostics (code, path, expected, hint) and applies nothing, so fix the patch and call again with the same markup. The result is validated strictly. Patch forms: {"op":"set","id","prop","value"} (value is typed JSON or null to remove; prop "on-<event>" sets an action name; prop "text" changes an element's text), {"op":"insert","parent","slot"?,"index"?,"markup"} (markup is one or more new elements), {"op":"remove","id"}, {"op":"move","id","parent","slot"?,"index"?}. Call weft_primer for details. At most ${LIMITS.patches} patches and ${LIMITS.patchesChars} characters of patches per call.`,
      inputSchema: {
        markup: markupSchema.describe("The current Weft markup (one <screen> document)."),
        // Items are checked by applyPatches, so a malformed patch gets a diagnostic that shows
        // the expected form instead of a generic schema error.
        patches: z
          .array(z.unknown())
          .max(LIMITS.patches)
          .describe(`Patches in application order, at most ${LIMITS.patches}.`),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, patches }) => {
      if (JSON.stringify(patches).length > LIMITS.patchesChars) {
        return failure(`The patch list is longer than ${LIMITS.patchesChars} characters.`);
      }
      const input = readMarkup(markup, context, "lenient");
      if (input.document === undefined) return failure(diagnosticsText(input.diagnostics));
      const { document, diagnostics } = applyPatches(input.document, patches, {
        ...context,
        mode: "strict",
      });
      if (document === undefined) return failure(diagnosticsText(diagnostics));
      return warned(serialize(document), diagnostics);
    }),
  );
}
