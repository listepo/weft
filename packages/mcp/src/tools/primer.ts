import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type { ServerSettings } from "../context.ts";
import { primer } from "../primer.ts";
import { text } from "../result.ts";

export function registerPrimer(
  server: McpServer,
  _context: unknown,
  { limits }: ServerSettings,
): void {
  server.registerTool(
    "weft_primer",
    {
      title: "Weft primer",
      description:
        "Read this first. Returns a short primer of the Weft UI markup format: syntax rules, how values and events are written, and how to use the other weft_* tools. Takes no arguments.",
      annotations: { readOnlyHint: true, idempotentHint: true, openWorldHint: false },
    },
    () => text(primer(limits)),
  );
}
