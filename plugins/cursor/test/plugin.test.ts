// The Cursor plugin's declarations match Cursor's published schemas and point at things that exist,
// and every feature runs from a copy of the plugin folder, which is all Cursor keeps in its cache
// (it extracts the marketplace entry's `source` folder, so nothing outside it is there).
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Ajv } from "ajv";
import { default as addFormats } from "ajv-formats";
import { afterAll, beforeAll, describe, test } from "vitest";
import { installCopy } from "../../shared/test/install.ts";
import { startServer, WEFT_TOOLS } from "../../shared/test/mcp-stdio.ts";

const ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PLUGIN = join(ROOT, "plugins/cursor");
const json = (path: string) => JSON.parse(readFileSync(path, "utf8"));
const text = (path: string) => readFileSync(path, "utf8");

/** The `key: value` lines of a file's frontmatter, or `undefined` when there is none. */
function frontmatter(source: string): Record<string, string> | undefined {
  const block = /^---\n([\s\S]*?)\n---\n/.exec(source)?.[1];
  if (block === undefined) return undefined;
  const fields: Record<string, string> = {};
  for (const line of block.split("\n")) {
    const at = line.indexOf(":");
    fields[line.slice(0, at)] = line.slice(at + 1).trim();
  }
  return fields;
}

// Cursor's own schemas, vendored from github.com/cursor/plugins (see the README).
const ajv = new Ajv({ allErrors: true });
addFormats.default(ajv);
const validatePlugin = ajv.compile(json(join(PLUGIN, "test/schemas/plugin.schema.json")));
const validateMarketplace = ajv.compile(json(join(PLUGIN, "test/schemas/marketplace.schema.json")));

describe(".cursor-plugin/marketplace.json", () => {
  const marketplace = json(join(ROOT, ".cursor-plugin/marketplace.json"));

  test("passes Cursor's marketplace schema", () => {
    assert.equal(
      validateMarketplace(marketplace),
      true,
      ajv.errorsText(validateMarketplace.errors),
    );
  });

  test("lists the plugin by the name its own manifest has", () => {
    const [entry, ...others] = marketplace.plugins;
    assert.equal(others.length, 0);
    assert.equal(entry.name, json(join(PLUGIN, ".cursor-plugin/plugin.json")).name);
  });

  test("points at the plugin folder with a relative path that stays inside the repository", () => {
    const { source } = marketplace.plugins[0];
    assert.equal(isAbsolute(source), false);
    assert.ok(!source.split("/").includes(".."));
    assert.equal(realpathSync(resolve(ROOT, source)), realpathSync(PLUGIN));
  });
});

describe(".cursor-plugin/plugin.json", () => {
  const manifest = json(join(PLUGIN, ".cursor-plugin/plugin.json"));

  test("passes Cursor's plugin schema", () => {
    assert.equal(validatePlugin(manifest), true, ajv.errorsText(validatePlugin.errors));
  });

  test("declares no component paths, so every default location applies", () => {
    // A path in the manifest replaces the folder's discovery, so none must hide a component.
    for (const key of ["skills", "rules", "commands", "agents", "hooks", "mcpServers"]) {
      assert.equal(key in manifest, false, key);
    }
  });

  test("matches the Claude Code plugin's identity", () => {
    const other = json(join(ROOT, "plugins/claude-code/.claude-plugin/plugin.json"));
    for (const key of ["name", "displayName", "version", "description", "license"]) {
      assert.equal(manifest[key], other[key], key);
    }
  });
});

describe("skills", () => {
  const skills = readdirSync(join(PLUGIN, "skills"));

  test("are weft-import, weft-export, weft-render, weft-figma-pull and weft-spec", () => {
    assert.deepEqual(skills.toSorted(), [
      "weft-export",
      "weft-figma-pull",
      "weft-import",
      "weft-render",
      "weft-spec",
    ]);
  });

  describe.each(skills)("%s", (dir) => {
    const source = text(join(PLUGIN, "skills", dir, "SKILL.md"));

    test("has a name equal to its folder and a description", () => {
      const fields = frontmatter(source);
      assert.equal(fields?.name, dir);
      assert.match(dir, /^[a-z0-9-]+$/);
      assert.ok((fields?.description ?? "").length > 20);
    });

    test("names only scripts that exist, by a path from its own folder", () => {
      const scripts = [...source.matchAll(/\.\.\/\.\.\/(dist\/[\w.-]+)/g)].map(
        (m) => m[1] as string,
      );
      for (const script of scripts) assert.ok(existsSync(join(PLUGIN, script)), script);
      if (dir !== "weft-spec") assert.ok(scripts.length > 0);
      // No host variable: Cursor documents none for skills.
      assert.doesNotMatch(source, /PLUGIN_ROOT/);
    });
  });

  test("weft-spec carries a real copy of the repository's AGENT-SPEC.md, since a link out of the folder does not survive the plugin cache", () => {
    const copy = join(PLUGIN, "skills/weft-spec/AGENT-SPEC.md");
    assert.equal(lstatSync(copy).isSymbolicLink(), false);
    assert.equal(text(copy), text(join(ROOT, "AGENT-SPEC.md")));
  });

  test("weft-render tells the agent to open the page in Cursor's built-in browser", () => {
    const source = text(join(PLUGIN, "skills/weft-render/SKILL.md"));
    assert.match(source, /built-in browser/);
    assert.match(source, /file:\/\//);
  });
});

describe("rules", () => {
  const rules = readdirSync(join(PLUGIN, "rules"));

  test("are .mdc files, the only rule format Cursor reads from a plugin", () => {
    assert.ok(rules.length > 0);
    for (const rule of rules) assert.match(rule, /\.mdc$/);
  });

  describe.each(rules)("%s", (file) => {
    const source = text(join(PLUGIN, "rules", file));
    const fields = frontmatter(source);

    test("is attached to .weft files and loaded on demand", () => {
      assert.equal(fields?.alwaysApply, "false");
      assert.equal(fields?.globs, '"**/*.weft"');
      assert.ok((fields?.description ?? "").length > 20);
    });

    test("stays under Cursor's 500-line advice and points at the guide and the tools", () => {
      assert.ok(source.split("\n").length < 500);
      assert.match(source, /AGENT-SPEC\.md/);
      assert.match(source, /weft_primer/);
    });
  });
});

describe("mcp.json", () => {
  const config = json(join(PLUGIN, "mcp.json"));

  test("registers the weft server as a stdio command", () => {
    const server = config.mcpServers.weft;
    assert.equal(server.type, "stdio");
    assert.equal(server.command, "node");
    assert.deepEqual(server.args, ["${CURSOR_PLUGIN_ROOT}/dist/server.js"]);
  });

  test("uses no variable but Cursor's plugin root, so none needs declaring in the manifest", () => {
    const names = [...text(join(PLUGIN, "mcp.json")).matchAll(/\$\{([^}]+)\}/g)].map((m) => m[1]);
    assert.deepEqual([...new Set(names)], ["CURSOR_PLUGIN_ROOT"]);
    assert.equal("variables" in json(join(PLUGIN, ".cursor-plugin/plugin.json")), false);
  });
});

describe("the plugin folder as Cursor caches it", () => {
  const scratch = mkdtempSync(join(tmpdir(), "weft-cursor-"));
  const copy = join(scratch, "cache/weft/weft/0.1.0");
  /** An empty directory that is neither the plugin nor the repository, like a user's project. */
  const project = join(scratch, "project");
  const corpus = join(ROOT, "corpus");
  // Coverage screens have no hand-written page (corpus/README.md); the import needs one.
  const screen = readdirSync(corpus, { withFileTypes: true }).find(
    (e) => e.isDirectory() && existsSync(join(corpus, e.name, "screen.html")),
  )?.name;
  assert.ok(screen);

  beforeAll(() => {
    installCopy("cursor", copy);
    mkdirSync(project, { recursive: true });
  });
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));

  /** Runs a script the way a skill says to: by the path from the skill's folder. */
  const run = (skill: string, script: string, ...args: string[]) =>
    spawnSync(
      process.execPath,
      [resolve(copy, "skills", skill, "../..", "dist", script), ...args],
      {
        cwd: project,
        encoding: "utf8",
      },
    );

  test("every path in the skills and in mcp.json resolves to a file of the copy", () => {
    const references = [
      ...readdirSync(join(copy, "skills")).flatMap((skill) =>
        [
          ...text(join(copy, "skills", skill, "SKILL.md")).matchAll(/\.\.\/\.\.\/dist\/[\w.-]+/g),
        ].map((m) => resolve(copy, "skills", skill, m[0])),
      ),
      ...json(join(copy, "mcp.json")).mcpServers.weft.args.map((arg: string) =>
        arg.replace("${CURSOR_PLUGIN_ROOT}", copy),
      ),
    ];
    assert.equal(references.length, 5);
    for (const reference of references) assert.ok(existsSync(reference), reference);
  });

  test("weft-import writes the screen and prints the loss table", () => {
    const out = join(project, "imported.weft");
    const result = run("weft-import", "import.js", join(corpus, screen!, "screen.html"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Import losses:/);
    assert.match(result.stdout, /\| Kind \| Path \| Note \|/);
    assert.ok(text(out).length > 0);
  });

  test("weft-export writes the React component", () => {
    const out = join(project, "Screen.jsx");
    const result = run("weft-export", "export.js", join(corpus, screen!, "screen.weft"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(text(out), /export default function/);
  });

  test("weft-render writes the page and prints the file URL the browser opens", () => {
    const out = join(project, "page.html");
    const data = join(corpus, screen!, "data.json");
    const result = run(
      "weft-render",
      "render.js",
      join(corpus, screen!, "screen.weft"),
      out,
      "--data",
      data,
    );
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /^file:\/\//m);
    assert.match(text(out), /<html/i);
  });

  test("a script that refuses reports it with exit code 1 and writes nothing", () => {
    const bad = join(project, "bad.weft");
    const out = join(project, "bad.jsx");
    const broken = text(join(corpus, screen!, "screen.weft")).replace(
      /variant="[^"]+"/,
      'variant="nope"',
    );
    mkdirSync(project, { recursive: true });
    writeFileSync(bad, broken);
    const result = run("weft-export", "export.js", bad, out);
    assert.equal(result.status, 1);
    assert.equal(existsSync(out), false);
  });

  test("the weft MCP server starts from mcp.json and serves its tools", async () => {
    const args = json(join(copy, "mcp.json")).mcpServers.weft.args.map((arg: string) =>
      arg.replace("${CURSOR_PLUGIN_ROOT}", copy),
    );
    const session = await startServer(args, project);
    try {
      assert.deepEqual((await session.tools()).toSorted(), WEFT_TOOLS);
      const markup = text(join(corpus, screen!, "screen.weft"));
      const validated = await session.call("weft_validate", { markup });
      assert.notEqual(validated.isError, true, validated.content[0]?.text ?? "");
    } finally {
      session.close();
    }
  });
});
