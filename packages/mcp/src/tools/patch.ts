import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { applyPatches, checkData, hasErrors, serialize } from "@weft/core";
import { z } from "zod";
import type { Context, ServerSettings } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { readMarkup } from "../read.ts";
import { diagnosticsText, failure, guarded } from "../result.ts";
import { warned } from "./format.ts";
import { markupSchema } from "./validate.ts";

export function registerPatch(server: McpServer, context: Context, settings: ServerSettings): void {
  const { limits } = settings;
  const contextRule =
    settings.context === "read-only"
      ? "This host keeps context read-only: context patches are refused (W512)."
      : 'Context forms: {"op":"add-context","entry":{"id","kind","by":"agent","name":your model id,"for"?,"status"?,"text"}}, {"op":"set-context","id","field":"text"|"kind"|"for","value"} (null clears for), {"op":"resolve-context","id"}, {"op":"remove-context","id"}; entries you add must say by "agent".';
  server.registerTool(
    "weft_patch",
    {
      title: "Patch Weft markup",
      description: `Edits existing Weft markup without rewriting it. Send the current markup and a list of patches addressed by element id; patches apply in order and all-or-nothing. On success it returns the new canonical markup; on any problem it returns diagnostics (code, path, expected, hint) and applies nothing, so fix the patch and call again with the same markup. The result is validated strictly. Patch forms: {"op":"set","id","prop","value"} (value is typed JSON or null to remove; prop "on-<event>" sets an action name; prop "text" changes an element's text), {"op":"insert","parent","slot"?,"index"?,"markup"} (markup is one or more new elements), {"op":"remove","id"}, {"op":"move","id","parent","slot"?,"index"?}. ${contextRule} Call weft_primer for details. At most ${limits.patches} patches and ${limits.patchesChars} characters of patches per call.`,
      inputSchema: {
        markup: markupSchema(limits).describe("The current Weft markup (one <screen> document)."),
        // Items are checked by applyPatches, so a malformed patch gets a diagnostic that shows
        // the expected form instead of a generic schema error.
        patches: z
          .array(z.unknown())
          .max(limits.patches)
          .describe(`Patches in application order, at most ${limits.patches}.`),
        project: projectSchema(limits),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ markup, patches, project }) => {
      if (JSON.stringify(patches).length > limits.patchesChars) {
        return failure(`The patch list is longer than ${limits.patchesChars} characters.`);
      }
      const scoped = scope(context, project, "strict", limits);
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      const fail = (all: Parameters<typeof diagnosticsText>[0]) =>
        failure(diagnosticsText(all, limits.diagnostics));
      if (scoped.context === undefined) return fail(scoped.diagnostics);
      const { data, ...options } = scoped.context;
      // The current markup only has to parse; the patched result is what gets validated.
      const input = readMarkup(markup, options, "lenient");
      if (input.document === undefined) return fail(input.diagnostics);
      // The agent channel cannot speak for a human (W512), whatever the patch claims.
      const { document, diagnostics } = applyPatches(input.document, patches, {
        ...options,
        mode: "strict",
        author: { by: "agent" },
        context: settings.context,
      });
      if (document === undefined) return fail(diagnostics);
      const all = [
        ...scoped.diagnostics,
        ...diagnostics,
        ...(data === undefined ? [] : checkData(document, { catalog: options.catalog, data })),
      ];
      if (hasErrors(all)) return fail(all);
      return warned(serialize(document), all, limits.diagnostics);
    }),
  );
}
