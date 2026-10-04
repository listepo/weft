import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { coreCatalog } from "@weft/catalog";
import { LIMITS, type Context, type Limits, type ServerSettings } from "./context.ts";
import { TOOLS } from "./tools/index.ts";

export type ServerOptions = {
  mode?: ServerSettings["mode"] | undefined;
  /** Limits to change; the rest keep their defaults (`LIMITS`). */
  limits?: Partial<Limits> | undefined;
};

export function createServer(
  overrides: Partial<Context> = {},
  options: ServerOptions = {},
): McpServer {
  const context: Context = { catalog: coreCatalog, ...overrides };
  const settings: ServerSettings = {
    limits: { ...LIMITS, ...options.limits },
    mode: options.mode ?? "lenient",
  };
  const server = new McpServer(
    { name: "weft", version: "0.1.0" },
    {
      maxToolInputElements: settings.limits.inputElements,
      instructions:
        "Weft is a UI description format. Call weft_primer first, then use weft_catalog, weft_validate, weft_format, weft_patch and weft_render. The server only works on markup you pass in; it reads no files.",
    },
  );
  for (const register of TOOLS) register(server, context, settings);
  return server;
}
