// Registering the weft MCP server and the agent guide with the agents built into Xcode. The real
// run goes to a temporary folder, never to ~/Library/Developer/Xcode, and needs the agents' own
// command lines; without one that agent's test skips with a message.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "vitest";
import { pluginDir, remove, repoRoot, run, tempDir } from "./helpers.ts";

const script = join(pluginDir, "scripts/xcode-agents.ts");
const server = join(repoRoot, "plugins/claude-code/dist/server.js");
const installed = (command: string) => {
  const found = spawnSync(command, ["--version"], { stdio: "ignore" }).status === 0;
  if (!found) console.warn(`skipped: the ${command} command line is not installed`);
  return found;
};

describe("Xcode agents", () => {
  const root = tempDir("agents");
  afterAll(() => remove(root));

  test("a dry run prints what it would do and writes nothing", () => {
    const result = run(process.execPath, [script, "--dry-run", "--root", root]);
    expect(result.status, result.output).toBe(0);
    expect(result.output).toContain(
      `CLAUDE_CONFIG_DIR=${join(root, "ClaudeAgentConfig")} claude mcp add`,
    );
    expect(result.output).toContain(`CODEX_HOME=${join(root, "codex")} codex mcp add`);
    expect(existsSync(join(root, "ClaudeAgentConfig"))).toBe(false);
  });

  test("an unknown agent is a usage error", () => {
    expect(run(process.execPath, [script, "--agents", "nobody", "--root", root]).status).toBe(2);
  });

  test("unknown agents are reported and the others still run", () => {
    const result = run(process.execPath, [
      script,
      "--dry-run",
      "--agents",
      "nobody,claude",
      "--root",
      root,
    ]);
    expect(result.status).toBe(2);
    expect(result.output).toContain('unknown agent "nobody"');
    expect(result.output).toContain("CLAUDE_CONFIG_DIR=");
  });

  test.skipIf(!installed("claude"))(
    "registers the server and the guide with the Claude agent",
    () => {
      const result = run(process.execPath, [script, "--agents", "claude", "--root", root]);
      expect(result.status, result.output).toBe(0);
      const folder = join(root, "ClaudeAgentConfig");
      const config = JSON.parse(readFileSync(join(folder, ".claude.json"), "utf8"));
      expect(config.mcpServers.weft).toMatchObject({
        type: "stdio",
        command: process.execPath,
        args: [server],
      });
      expect(existsSync(join(folder, "skills/weft-spec/AGENT-SPEC.md"))).toBe(true);
      // Running it again replaces the entry instead of failing.
      expect(run(process.execPath, [script, "--agents", "claude", "--root", root]).status).toBe(0);
    },
  );

  test.skipIf(!installed("codex"))(
    "registers the server and the guide with the Codex agent",
    () => {
      const result = run(process.execPath, [script, "--agents", "codex", "--root", root]);
      expect(result.status, result.output).toBe(0);
      const folder = join(root, "codex");
      const config = readFileSync(join(folder, "config.toml"), "utf8");
      expect(config).toContain("[mcp_servers.weft]");
      expect(config).toContain(JSON.stringify(server));
      expect(existsSync(join(folder, "skills/weft-spec/AGENT-SPEC.md"))).toBe(true);
      expect(run(process.execPath, [script, "--agents", "codex", "--root", root]).status).toBe(0);
    },
  );
});
