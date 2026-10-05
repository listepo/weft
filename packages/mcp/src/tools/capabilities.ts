import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { coreCatalog } from "@weft/catalog";
import type { Context, ServerSettings } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";

/** Names listed per category; a project's token set can be far larger than a model should read. */
export const LISTED = 200;

const listed = (names: readonly string[]) => ({
  count: names.length,
  names: names.toSorted().slice(0, LISTED),
  ...(names.length > LISTED ? { truncated: true } : {}),
});

/**
 * What a host advertises (SPEC §8): the format version and catalogs a writer may use, and, when
 * the host checks them, the token paths and action names it knows. A category the host does not
 * check is left out, which tells a writer nothing is enforced there.
 */
export function capabilitiesOf({ catalog, tokens, actions, data }: Context) {
  // An extension is merged over the core catalog and takes the extension's name and version, so
  // the core one is named as well: every core component is still there.
  const catalogs = [...(catalog.name === coreCatalog.name ? [] : [coreCatalog]), catalog].map(
    ({ name, version }) => ({ name, version }),
  );
  return {
    weft: catalog.weft,
    catalogs,
    ...(tokens === undefined ? {} : { tokens: listed([...tokens.keys()]) }),
    ...(actions === undefined ? {} : { actions: listed(actions) }),
    ...(data === undefined ? {} : { data: true }),
  };
}

export function registerCapabilities(
  server: McpServer,
  context: Context,
  { limits }: ServerSettings,
): void {
  server.registerTool(
    "weft_capabilities",
    {
      title: "Weft host capabilities",
      description:
        'Reports what this host accepts, as JSON: {"weft":"0.1","catalogs":[{"name","version"}],"tokens":{"count","names"},"actions":{"count","names"},"data":true}. Write only the format version and catalogs listed, and only the design tokens and actions named. tokens, actions and data appear only when the host checks them; without one, that kind of reference is not checked. Lists are cut at ' +
        `${LISTED} names (then "truncated":true). Pass project to see the capabilities of that project instead.`,
      inputSchema: { project: projectSchema(limits) },
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    guarded(({ project }) => {
      const scoped = scope(context, project, "lenient", limits);
      if ("tooLong" in scoped) return failure(scoped.tooLong);
      if (scoped.context === undefined)
        return failure(diagnosticsText(scoped.diagnostics, limits.diagnostics));
      return text(JSON.stringify(capabilitiesOf(scoped.context)));
    }),
  );
}
