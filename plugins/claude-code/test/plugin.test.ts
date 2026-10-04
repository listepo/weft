// The plugin's declarations point at things that exist, and the MCP server it registers starts.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, test } from "vitest";

const ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PLUGIN = join(ROOT, "plugins/claude-code");
const json = (path: string) => JSON.parse(readFileSync(path, "utf8"));
/** The path a skill or server config means once Claude Code has substituted the plugin root. */
const substitute = (value: string) => value.replaceAll("${CLAUDE_PLUGIN_ROOT}", PLUGIN);

describe("marketplace.json", () => {
  const marketplace = json(join(ROOT, ".claude-plugin/marketplace.json"));

  test("lists the plugin by the name its own manifest has", () => {
    const [entry, ...others] = marketplace.plugins;
    assert.equal(others.length, 0);
    assert.equal(entry.name, json(join(PLUGIN, ".claude-plugin/plugin.json")).name);
  });

  test("points at the plugin folder with a relative path that stays inside the repository", () => {
    const { source } = marketplace.plugins[0];
    assert.match(source, /^\.\//);
    assert.ok(!source.includes(".."));
    assert.equal(realpathSync(resolve(ROOT, source)), realpathSync(PLUGIN));
  });

  test("names itself and its owner", () => {
    assert.equal(typeof marketplace.name, "string");
    assert.equal(typeof marketplace.owner.name, "string");
  });
});

describe("plugin.json", () => {
  const manifest = json(join(PLUGIN, ".claude-plugin/plugin.json"));

  test("has a kebab-case name that does not pass as one of Anthropic's own", () => {
    assert.match(manifest.name, /^[a-z][a-z0-9-]*$/);
    assert.doesNotMatch(manifest.name, /^(claude|anthropic)/);
  });

  test("declares no component paths, so every default location applies", () => {
    for (const key of ["skills", "commands", "agents", "hooks", "mcpServers"]) {
      assert.equal(key in manifest, false, key);
    }
  });
});

describe("skills", () => {
  const skills = readdirSync(join(PLUGIN, "skills"));

  test("are import, export, render and spec", () => {
    assert.deepEqual(skills.toSorted(), ["export", "import", "render", "spec"]);
  });

  describe.each(skills)("%s", (name) => {
    const text = readFileSync(join(PLUGIN, "skills", name, "SKILL.md"), "utf8");

    test("has frontmatter with a description", () => {
      assert.match(text, /^---\ndescription: .+\n(argument-hint: .+\n)?---\n/);
    });

    test("runs only scripts that exist", () => {
      const scripts = [...text.matchAll(/\$\{CLAUDE_PLUGIN_ROOT\}\/(scripts\/[\w.-]+)/g)].map(
        (match) => match[1] as string,
      );
      for (const script of scripts) assert.ok(existsSync(join(PLUGIN, script)), script);
      if (name !== "spec") assert.ok(scripts.length > 0);
    });
  });

  test("spec reads the repository's AGENT-SPEC.md through a link, not a copy", () => {
    const link = join(PLUGIN, "skills/spec/AGENT-SPEC.md");
    assert.equal(realpathSync(link), realpathSync(join(ROOT, "AGENT-SPEC.md")));
  });

  test("render tells Claude to open the page in the Desktop browser", () => {
    const text = readFileSync(join(PLUGIN, "skills/render/SKILL.md"), "utf8");
    assert.match(text, /built-in browser/);
    assert.match(text, /file:\/\//);
  });
});

describe(".mcp.json", () => {
  const config = json(join(PLUGIN, ".mcp.json"));

  test("registers the weft server, started from a file that exists", () => {
    const server = config.mcpServers.weft;
    assert.equal(server.command, "node");
    const [entry] = server.args.map(substitute);
    assert.ok(existsSync(entry), entry);
  });

  test("the server starts and serves the weft tools over stdio", async () => {
    const server = config.mcpServers.weft;
    const child = spawn(process.execPath, server.args.map(substitute), {
      cwd: dirname(PLUGIN),
      stdio: ["pipe", "pipe", "inherit"],
    });
    try {
      const names = await new Promise<string[]>((resolveNames, reject) => {
        let buffer = "";
        child.on("error", reject);
        child.on("exit", () => reject(new Error("the server exited before it answered")));
        child.stdout.on("data", (chunk: Buffer) => {
          buffer += chunk;
          for (const line of buffer.split("\n")) {
            const message = line.trim() === "" ? undefined : JSON.parse(line);
            if (message?.id === 2)
              resolveNames(message.result.tools.map((t: { name: string }) => t.name));
          }
        });
        const send = (message: object) => child.stdin.write(`${JSON.stringify(message)}\n`);
        send({
          jsonrpc: "2.0",
          id: 1,
          method: "initialize",
          params: {
            protocolVersion: "2025-06-18",
            capabilities: {},
            clientInfo: { name: "plugin-test", version: "0" },
          },
        });
        send({ jsonrpc: "2.0", method: "notifications/initialized" });
        send({ jsonrpc: "2.0", id: 2, method: "tools/list" });
      });
      assert.deepEqual(names.toSorted(), [
        "weft_catalog",
        "weft_format",
        "weft_patch",
        "weft_primer",
        "weft_render",
        "weft_validate",
      ]);
    } finally {
      child.kill();
    }
  });
});
