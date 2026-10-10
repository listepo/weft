import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { coreCatalog } from "@weft/catalog";
import type { Context, ServerSettings } from "../context.ts";
import { projectSchema, scope } from "../project.ts";
import { diagnosticsText, failure, guarded, text } from "../result.ts";

type CatalogEntry = { name: string; version: string; prefix?: string | undefined; source?: string };

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
export function capabilitiesOf({ catalog, catalogs: loaded, tokens, actions, data }: Context) {
  // Every catalog is merged over the core, so the core comes first. A host that hands over one
  // merged catalog, with no list, is named by that catalog.
  const given = loaded ?? (catalog.name === coreCatalog.name ? [] : [catalog]);
  const catalogs = [
    { name: coreCatalog.name, version: coreCatalog.version },
    ...given.map(({ name, version, prefix, source }: CatalogEntry) => ({
      name,
      version,
      ...(prefix === undefined ? {} : { prefix }),
      ...(source === undefined ? {} : { source }),
    })),
  ];
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
        'Reports what this host accepts, as JSON: {"weft":"0.1","catalogs":[{"name","version","prefix","source"}],"tokens":{"count","names"},"actions":{"count","names"},"data":true}. catalogs lists the core first, then each catalog merged over it in load order; a library has a prefix and owns the kinds named <prefix>-…, which are ordinary catalog kinds, unlike x- extensions; source is the file or package it came from when the host knows it. Write only the format version and catalogs listed, and only the design tokens and actions named. tokens, actions and data appear only when the host checks them; without one, that kind of reference is not checked. Lists are cut at ' +
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
