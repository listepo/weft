#!/usr/bin/env node
// Registers the weft MCP server and the agent guide (AGENT-SPEC) with the coding agents built into
// Xcode. Xcode runs each agent with its own configuration folder under
// ~/Library/Developer/Xcode/CodingAssistant, which is why registering with the agent's own command
// line in your home folder does not reach it. The agents' own `mcp add` commands write the right
// file in the right format, so this script runs them with the folder pointed at Xcode's.
//
//   node scripts/xcode-agents.ts [--dry-run] [--agents claude,codex] [--root <folder>]
//
// An agent whose command line is not installed is skipped with a message.
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const here = dirname(fileURLToPath(import.meta.url));
const claudePlugin = resolve(here, "../../claude-code");

interface Agent {
  /** The folder Apple documents for the agent, under the root. */
  folder: string;
  /** The environment variable that makes the agent read that folder (Xcode sets the same one). */
  variable: string;
  command: string;
  add: (node: string, server: string) => string[];
  remove: string[];
}

const AGENTS: Record<string, Agent> = {
  claude: {
    folder: "ClaudeAgentConfig",
    variable: "CLAUDE_CONFIG_DIR",
    command: "claude",
    add: (node, server) => [
      "mcp",
      "add",
      "--scope",
      "user",
      "--transport",
      "stdio",
      "weft",
      "--",
      node,
      server,
    ],
    remove: ["mcp", "remove", "--scope", "user", "weft"],
  },
  codex: {
    folder: "codex",
    variable: "CODEX_HOME",
    command: "codex",
    add: (node, server) => ["mcp", "add", "weft", "--", node, server],
    remove: ["mcp", "remove", "weft"],
  },
};

const { values } = parseArgs({
  options: {
    "dry-run": { type: "boolean", default: false },
    agents: { type: "string", default: "claude,codex" },
    root: { type: "string", default: join(homedir(), "Library/Developer/Xcode/CodingAssistant") },
  },
  strict: true,
});

const root = resolve(values.root);
const dryRun = values["dry-run"];
// An absolute node: Xcode starts the agent, and the agent starts the server, with whatever PATH
// Xcode has, which may not hold the node that runs this script.
const node = process.execPath;
const server = join(claudePlugin, "dist/server.js");
const skill = join(claudePlugin, "skills/spec");

let failed = false;
for (const name of values.agents.split(",")) {
  const agent = AGENTS[name];
  if (!agent) {
    console.error(`unknown agent "${name}" (known: ${Object.keys(AGENTS).join(", ")})`);
    process.exit(2);
  }
  const folder = join(root, agent.folder);
  const env = { ...process.env, [agent.variable]: folder };
  const show = (args: string[]) => `${agent.variable}=${folder} ${agent.command} ${args.join(" ")}`;
  if (dryRun) {
    console.log(show(agent.add(node, server)));
    console.log(`copy ${skill} to ${join(folder, "skills/weft-spec")}`);
    continue;
  }
  const found = spawnSync(agent.command, ["--version"], { stdio: "ignore" });
  if (found.error || found.status !== 0) {
    console.log(`skipped ${name}: the ${agent.command} command line is not installed`);
    continue;
  }
  mkdirSync(folder, { recursive: true });
  // Adding twice is an error, and a stale entry may point at an old checkout.
  spawnSync(agent.command, agent.remove, { env, stdio: "ignore" });
  const added = spawnSync(agent.command, agent.add(node, server), { env, stdio: "inherit" });
  if (added.status !== 0) {
    console.error(`failed: ${show(agent.add(node, server))}`);
    failed = true;
    continue;
  }
  const target = join(folder, "skills/weft-spec");
  rmSync(target, { recursive: true, force: true });
  cpSync(skill, target, { recursive: true });
  console.log(`registered weft with the ${name} agent of Xcode (${folder})`);
}
process.exit(failed ? 1 : 0);
