// Calls one tool of the Weft MCP server and prints the text it returns, so the tools can be tried
// without an MCP client (docs/mcp.md):
//   node docs/examples/mcp-call.mjs <tool> [name=value ...]
// A value is read as JSON when it parses, as a string otherwise; "@file" reads the file instead
// (parsed as JSON when its name ends in .json). Exit code 1 when the tool reports an error.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline";

const [tool, ...pairs] = process.argv.slice(2);
const args = {};
for (const pair of pairs) {
  const at = pair.indexOf("=");
  const name = pair.slice(0, at);
  const value = pair.slice(at + 1);
  if (value.startsWith("@")) {
    const text = readFileSync(value.slice(1), "utf8");
    args[name] = value.endsWith(".json") ? JSON.parse(text) : text;
  } else {
    try {
      args[name] = JSON.parse(value);
    } catch {
      args[name] = value;
    }
  }
}

const server = spawn(
  process.execPath,
  [fileURLToPath(new URL("../../packages/mcp/src/server.ts", import.meta.url))],
  {
    stdio: ["pipe", "pipe", "inherit"],
  },
);
const send = (message) => server.stdin.write(`${JSON.stringify(message)}\n`);
createInterface({ input: server.stdout }).on("line", (line) => {
  const message = JSON.parse(line);
  if (message.id !== 2) return;
  for (const part of message.result.content) {
    process.stdout.write(part.text.endsWith("\n") ? part.text : `${part.text}\n`);
  }
  server.kill();
  process.exitCode = message.result.isError ? 1 : 0;
});
send({
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: {
    protocolVersion: "2025-06-18",
    capabilities: {},
    clientInfo: { name: "mcp-call", version: "0" },
  },
});
send({ jsonrpc: "2.0", method: "notifications/initialized" });
send({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: tool, arguments: args } });
