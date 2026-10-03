import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { didYouMean } from "@weft/core";
import { z } from "zod";
import type { Context } from "../context.ts";
import { failure, guarded, text } from "../result.ts";

export function registerCatalog(server: McpServer, context: Context): void {
  const { catalog } = context;
  server.registerTool(
    "weft_catalog",
    {
      title: "Weft catalog",
      description:
        "Lists the components you may use in Weft markup. Without arguments it returns a compact index (kind, ARIA role, content model, one-line description) of every component; with `kind` it returns the full definition of that one component: props with types, defaults and allowed values, slots, states, events, and parent/child rules. Prefer the index first, then look up only the kinds you need.",
      inputSchema: {
        kind: z
          .string()
          .max(100)
          .optional()
          .describe('Component kind such as "button". Omit for the index.'),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ kind }) => {
      if (kind === undefined) {
        const lines = Object.entries(catalog.components).map(
          ([name, def]) => `${name} | ${def.role} | ${def.content} | ${def.description}`,
        );
        return text(
          [
            `catalog ${catalog.name} ${catalog.version} (weft ${catalog.weft}); kind | role | content | description`,
            ...lines,
          ].join("\n"),
        );
      }
      if (!Object.hasOwn(catalog.components, kind)) {
        const hint = didYouMean(kind, Object.keys(catalog.components));
        return failure(
          `Unknown component ${JSON.stringify(kind)}.${hint === undefined ? "" : ` ${hint}`} Call weft_catalog without arguments for the list.`,
        );
      }
      return text(JSON.stringify({ kind, ...catalog.components[kind] }));
    }),
  );
}
