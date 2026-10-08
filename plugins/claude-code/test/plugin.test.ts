// The plugin's declarations point at things that exist, and the MCP server it registers starts.
import assert from "node:assert/strict";
import {
  existsSync,
  lstatSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, test } from "vitest";
import { installCopy } from "../../shared/test/install.ts";
import { startServer, WEFT_TOOLS } from "../../shared/test/mcp-stdio.ts";

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

  test("are import, export, render, figma-pull and spec", () => {
    assert.deepEqual(skills.toSorted(), ["export", "figma-pull", "import", "render", "spec"]);
  });

  describe.each(skills)("%s", (name) => {
    const text = readFileSync(join(PLUGIN, "skills", name, "SKILL.md"), "utf8");

    test("has frontmatter with a description", () => {
      assert.match(text, /^---\ndescription: .+\n(argument-hint: .+\n)?---\n/);
    });

    test("runs only scripts that exist", () => {
      const scripts = [...text.matchAll(/\$\{CLAUDE_PLUGIN_ROOT\}\/(dist\/[\w.-]+)/g)].map(
        (match) => match[1] as string,
      );
      for (const script of scripts) assert.ok(existsSync(join(PLUGIN, script)), script);
      if (name !== "spec") assert.ok(scripts.length > 0);
    });
  });

  test("spec carries a real copy of the repository's AGENT-SPEC.md, since a link out of the folder does not survive the plugin cache", () => {
    const copy = join(PLUGIN, "skills/spec/AGENT-SPEC.md");
    assert.equal(lstatSync(copy).isSymbolicLink(), false);
    assert.equal(readFileSync(copy, "utf8"), readFileSync(join(ROOT, "AGENT-SPEC.md"), "utf8"));
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
    const session = await startServer(config.mcpServers.weft.args.map(substitute), dirname(PLUGIN));
    try {
      assert.deepEqual((await session.tools()).toSorted(), WEFT_TOOLS);
    } finally {
      session.close();
    }
  });
});

describe("the plugin folder as Claude Code caches it", () => {
  const scratch = mkdtempSync(join(tmpdir(), "weft-claude-code-"));
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));
  const copy = installCopy("claude-code", join(scratch, "weft/0.1.0"));
  const inCopy = (value: string) => value.replaceAll("${CLAUDE_PLUGIN_ROOT}", copy);

  test("every skill and the MCP config point at a file of the copy", () => {
    const references = [
      ...["export", "import", "render", "figma-pull"].flatMap((name) => {
        const text = readFileSync(join(copy, "skills", name, "SKILL.md"), "utf8");
        return [...text.matchAll(/\$\{CLAUDE_PLUGIN_ROOT\}\/[\w./-]+/g)].map((m) => m[0]);
      }),
      ...json(join(copy, ".mcp.json")).mcpServers.weft.args,
    ];
    assert.equal(references.length, 5);
    for (const reference of references) assert.ok(existsSync(inCopy(reference)), reference);
  });
});
