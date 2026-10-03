import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type { Context } from "../context.ts";
import { registerCatalog } from "./catalog.ts";
import { registerFormat } from "./format.ts";
import { registerPatch } from "./patch.ts";
import { registerPrimer } from "./primer.ts";
import { registerRender } from "./render.ts";
import { registerValidate } from "./validate.ts";

/** A tool is one `(server, context)` registration. */
export const TOOLS: readonly ((server: McpServer, context: Context) => void)[] = [
  registerPrimer,
  registerCatalog,
  registerValidate,
  registerFormat,
  registerPatch,
  registerRender,
];
