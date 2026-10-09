// A minimal MCP client over stdio, enough to start a server as an agent host does and call its tools.
import { spawn } from "node:child_process";

export type McpResult = { content: { type: string; text: string }[]; isError?: boolean };

export type McpSession = {
  tools: () => Promise<string[]>;
  call: (name: string, args: object) => Promise<McpResult>;
  close: () => void;
};

/** Starts `node <args>` in `cwd` and completes the MCP handshake. */
export async function startServer(args: string[], cwd: string): Promise<McpSession> {
  const child = spawn(process.execPath, args, { cwd, stdio: ["pipe", "pipe", "inherit"] });
  const pending = new Map<number, (message: { result?: unknown; error?: unknown }) => void>();
  let buffer = "";
  const failAll = (reason: Error) => {
    for (const settle of pending.values()) settle({ error: reason.message });
    pending.clear();
  };
  child.on("error", failAll);
  child.on("exit", () => failAll(new Error("the server exited before it answered")));
  child.stdout.on("data", (chunk: Buffer) => {
    buffer += chunk;
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      if (line.trim() === "") continue;
      const message = JSON.parse(line);
      pending.get(message.id)?.(message);
      pending.delete(message.id);
    }
  });

  let nextId = 1;
  const request = (method: string, params: object = {}) =>
    new Promise<unknown>((resolve, reject) => {
      const id = nextId++;
      pending.set(id, (message) =>
        message.error === undefined
          ? resolve(message.result)
          : reject(new Error(String(message.error))),
      );
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });

  await request("initialize", {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "plugin-test", version: "0" },
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
  return {
    tools: async () =>
      ((await request("tools/list")) as { tools: { name: string }[] }).tools.map((t) => t.name),
    call: async (name, toolArgs) =>
      (await request("tools/call", { name, arguments: toolArgs })) as McpResult,
    close: () => child.kill(),
  };
}

export const WEFT_TOOLS = [
  "weft_capabilities",
  "weft_catalog",
  "weft_context",
  "weft_format",
  "weft_patch",
  "weft_primer",
  "weft_render",
  "weft_schema",
  "weft_validate",
];
