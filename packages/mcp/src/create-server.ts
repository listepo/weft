import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";
import { coreCatalog } from "@weft/catalog";
import type { Document } from "@weft/core";
import { toolHost, type ToolResult } from "@weft/core/tools";
import { renderPage } from "@weft/render-react";
import { z } from "zod";
import type { Context, Limits, ServerSettings } from "./context.ts";

export type ServerOptions = {
  mode?: ServerSettings["mode"] | undefined;
  /** Limits to change; the rest keep their defaults (`LIMITS`). */
  limits?: Partial<Limits> | undefined;
  context?: ServerSettings["context"] | undefined;
};

/**
 * `weft_render` with `html`'s last step: the Rust tools have checked the markup strictly and built
 * the accessibility tree, and the reference renderer draws the page. The tool contract is "never
 * throw", so whatever escapes the renderer becomes an error result.
 */
function finish(result: ToolResult): CallToolResult {
  if (!("render" in result)) return result;
  try {
    const { tree, catalog, data } = result.render;
    const document = result.render.document as Document;
    return {
      content: [
        { type: "text", text: tree },
        { type: "text", text: renderPage(document, { catalog, data }) },
      ],
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { content: [{ type: "text", text: `Internal error: ${message}` }], isError: true };
  }
}

export function createServer(
  overrides: Partial<Context> = {},
  options: ServerOptions = {},
): McpServer {
  const context: Context = { catalog: coreCatalog, ...overrides };
  // The settings in the shape of weft.json's tool sections (SPEC §10.6), as the Rust host reads them.
  const host = toolHost(context, {
    validate: { mode: options.mode ?? "lenient" },
    mcp: { limits: options.limits, context: options.context ?? "read-write" },
  });
  const server = new McpServer(
    { name: "weft", version: "0.1.0" },
    {
      maxToolInputElements: (host.constants().limits as Limits).inputElements,
      instructions:
        "Weft is a UI description format. Call weft_primer first, then weft_capabilities to see which catalogs, tokens and actions this host accepts, and use weft_catalog, weft_validate, weft_format, weft_patch, weft_render and weft_context. The server only works on markup you pass in; it reads no files.",
    },
  );
  for (const { name, title, description, inputSchema, annotations } of host.decls()) {
    const call = (args: Record<string, unknown>) => finish(host.call(name, args));
    const properties = inputSchema["properties"] as Record<string, unknown> | undefined;
    if (properties === undefined || Object.keys(properties).length === 0) {
      server.registerTool(name, { title, description, annotations }, () => call({}));
    } else {
      // The SDK checks the arguments against the declared schema before the call reaches Rust.
      // A plain object of the same shape: unknown members are dropped, as before, rather than
      // passed (the loose object JSON Schema converts to would list `additionalProperties`).
      const input = z.object((z.fromJSONSchema(inputSchema) as z.ZodObject).shape);
      server.registerTool(name, { title, description, inputSchema: input, annotations }, call);
    }
  }
  return server;
}
