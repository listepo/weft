// The Weft agent tools of crates/weft-binding (`tools`), which the MCP server of @weft/mcp serves
// and the cox plugin calls natively, so both hosts answer with the same texts and checks. In the
// web module, like `./document-schema.ts`: `@weft/core` keeps loading only the core module.
import type { DataSchema } from "./data.ts";
import type { Catalog } from "./model.ts";
import { toJson, wasm } from "./web.ts";

/** What every tool judges markup with; the host decides, never the model. */
export type ToolContext = {
  catalog: Catalog;
  catalogs?: readonly unknown[] | undefined;
  kinds?: Readonly<Record<string, unknown>> | undefined;
  /** Token path → DTCG `$type`. */
  tokens?: ReadonlyMap<string, string> | undefined;
  actions?: readonly string[] | undefined;
  data?: DataSchema | undefined;
};

/** The tool sections of `weft.json` (SPEC §10.6), which are the host's settings. */
export type ToolSettings = {
  validate?: { mode?: "lenient" | "strict" | undefined } | undefined;
  mcp?: { limits?: Record<string, number> | undefined; context?: string | undefined } | undefined;
};

export type ToolDecl = {
  name: string;
  title: string;
  description: string;
  inputSchema: Record<string, unknown>;
  annotations: { readOnlyHint: boolean; idempotentHint: boolean; openWorldHint: boolean };
};

export type ToolContent = { type: "text"; text: string }[];

/**
 * A call's result: text blocks, or for `weft_render` with `html` the accessibility tree (the
 * first block) and the checked document whose page the host draws as the second.
 */
export type ToolResult =
  | { content: ToolContent; isError?: true }
  | { render: { tree: string; document: unknown; catalog: Catalog; data: unknown } };

export type ToolConstants = {
  limits: Record<string, number>;
  listed: number;
  contextNotice: string;
  primer: string;
};

export type ToolHost = {
  decls(): ToolDecl[];
  call(name: string, args: Record<string, unknown>): ToolResult;
  constants(): ToolConstants;
};

export function toolHost(context: ToolContext, settings: ToolSettings = {}): ToolHost {
  const { catalog, catalogs, kinds, tokens, actions, data } = context;
  const wire = {
    catalog,
    catalogs,
    kinds,
    // Entries, so that the order of the Map survives.
    tokens: tokens && [...tokens],
    actions,
    data: data?.json === undefined ? undefined : (JSON.parse(data.json) as unknown),
  };
  const host = new wasm.ToolHost(toJson(wire) ?? "{}", JSON.stringify(settings));
  return {
    decls: () => JSON.parse(host.decls()) as ToolDecl[],
    call: (name, args) => JSON.parse(host.call(name, toJson(args))) as ToolResult,
    constants: () => JSON.parse(host.constants()) as ToolConstants,
  };
}
