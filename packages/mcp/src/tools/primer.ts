import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { PRIMER } from "../primer.ts";
import { text } from "../result.ts";

export function registerPrimer(server: McpServer): void {
  server.registerTool(
    "weft_primer",
    {
      title: "Weft primer",
      description:
        "Read this first. Returns a short primer of the Weft UI markup format: syntax rules, how values and events are written, and how to use the other weft_* tools. Takes no arguments.",
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    () => text(PRIMER),
  );
}
