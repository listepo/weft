import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { coreCatalog } from "@weft/catalog";
import { LIMITS, type Context } from "./context.ts";
import { TOOLS } from "./tools/index.ts";

export function createServer(overrides: Partial<Context> = {}): McpServer {
  const context: Context = { catalog: coreCatalog, ...overrides };
  const server = new McpServer(
    { name: "weft", version: "0.1.0" },
    {
      maxToolInputElements: LIMITS.inputElements,
      instructions:
        "Weft is a UI description format. Call weft_primer first, then use weft_catalog, weft_validate, weft_format and weft_patch. The server only works on markup you pass in; it reads no files.",
    },
  );
  for (const register of TOOLS) register(server, context);
  return server;
}
