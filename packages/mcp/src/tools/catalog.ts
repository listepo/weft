import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { didYouMean, serialize, type Document } from "@weft/core";
import { z } from "zod";
import type { Context, ServerSettings } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";

export function registerCatalog(
  server: McpServer,
  context: Context,
  { limits }: ServerSettings,
): void {
  server.registerTool(
    "weft_catalog",
    {
      title: "Weft catalog",
      description:
        "Lists the components you may use in Weft markup. Without arguments it returns a compact index (kind, ARIA role, content model, one-line description) of every component; with `kind` it returns the full definition of that one component: props with types, defaults and allowed values, slots, states, events, and parent/child rules. Prefer the index first, then look up only the kinds you need. Pass project to list the project's catalog: the core components with its extension merged in, then the project's fragments, placed with <use id fragment>, with their parameters; pass a fragment's name as kind for its markup.",
      inputSchema: {
        kind: z
          .string()
          .max(100)
          .optional()
          .describe('Component kind such as "button". Omit for the index.'),
        project: projectSchema(limits),
      },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ kind, project }) => {
      const scoped = scope(context, project, "lenient", limits);
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      if (scoped.context === undefined)
        return failure(diagnosticsText(scoped.diagnostics, limits.diagnostics));
      const { catalog } = scoped.context;
      if (kind === undefined) {
        const lines = Object.entries(catalog.components).map(
          ([name, def]) => `${name} | ${def.role} | ${def.content} | ${def.description}`,
        );
        const fragments = Object.entries(catalog.fragments ?? {}).map(
          ([name, fragment]) => `fragment ${name} | ${fragmentParams(fragment).join(", ")}`,
        );
        return text(
          [
            `catalog ${catalog.name} ${catalog.version} (weft ${catalog.weft}); kind | role | content | description`,
            ...lines,
            ...(fragments.length > 0
              ? ["fragments; <use> one as fragment | parameters", ...fragments]
              : []),
          ].join("\n"),
        );
      }
      const fragment = catalog.fragments?.[kind];
      if (fragment !== undefined && !Object.hasOwn(catalog.components, kind)) {
        return text(serialize(fragment));
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

/** A fragment's parameters as `name: type`, required ones marked (SPEC §10.7). */
function fragmentParams(fragment: Document): string[] {
  const out: string[] = [];
  for (const child of fragment.root.children ?? []) {
    if (typeof child === "string" || child.kind !== "param") break;
    const { name, type, required } = child.props ?? {};
    out.push(`${String(name)}: ${String(type)}${required === true ? " (required)" : ""}`);
  }
  return out;
}
