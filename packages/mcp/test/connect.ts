import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { createServer, type Context, type ServerOptions } from "../src/index.ts";

export async function connect(overrides: Partial<Context> = {}, options: ServerOptions = {}) {
  return connectTo(createServer(overrides, options));
}

export async function connectTo(server: McpServer) {
  const [serverSide, clientSide] = InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "test", version: "0.0.0" });
  await Promise.all([server.connect(serverSide), client.connect(clientSide)]);
  return { client, close: () => client.close() };
}

export type Called = { isError: boolean; blocks: string[] };

/** Flattens a tool result into plain data so assertions do not depend on SDK types. */
export async function call(
  client: Client,
  name: string,
  args: Record<string, unknown>,
): Promise<Called> {
  const result = await client.callTool({ name, arguments: args });
  const content = result["content"] as { type: string; text: string }[];
  return {
    isError: result["isError"] === true,
    blocks: content.map((block) => block.text),
  };
}
