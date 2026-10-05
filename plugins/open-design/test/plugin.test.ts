// The Open Design plugin's declarations match Open Design's published manifest schema and point at
// things that exist, and every feature runs from a copy of the plugin folder: `od plugin install`
// copies the whole folder into the daemon's registry and the host stages the skill folder in the
// project, so nothing outside it is there at run time.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Ajv2020 } from "ajv/dist/2020.js";
import { default as addFormats } from "ajv-formats";
import { afterAll, beforeAll, describe, test } from "vitest";
import { parse } from "yaml";
import { loadTokens } from "@weft/catalog";
import { installCopy } from "../../shared/test/install.ts";
import { startServer, WEFT_TOOLS } from "../../shared/test/mcp-stdio.ts";

const ROOT = fileURLToPath(new URL("../../..", import.meta.url));
const PLUGIN = join(ROOT, "plugins/open-design");
const DESIGN_SYSTEMS = join(ROOT, "packages/design-md/test/fixtures");
const json = (path: string) => JSON.parse(readFileSync(path, "utf8"));
const text = (path: string) => readFileSync(path, "utf8");

/** The folder's files, without the workspace's `node_modules`, as paths relative to it. */
const filesIn = (dir: string): string[] =>
  readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => !entry.parentPath.split("/").includes("node_modules"))
    .filter((entry) => entry.name !== "node_modules")
    .map((entry) => relative(dir, join(entry.parentPath, entry.name)))
    .toSorted();

function frontmatter(source: string): Record<string, unknown> | undefined {
  const block = /^---\n([\s\S]*?)\n---\n/.exec(source)?.[1];
  return block === undefined ? undefined : (parse(block) as Record<string, unknown>);
}

// Open Design's own schema, vendored from github.com/nexu-io/open-design (see the NOTICE).
const ajv = new Ajv2020({ allErrors: true });
addFormats.default(ajv);
const validateManifest = ajv.compile(json(join(PLUGIN, "test/schemas/open-design.plugin.v1.json")));

describe("open-design.json", () => {
  const manifest = json(join(PLUGIN, "open-design.json"));

  test("passes Open Design's manifest schema", () => {
    assert.equal(validateManifest(manifest), true, ajv.errorsText(validateManifest.errors));
  });

  test("declares the plugin the way the specification asks of a skill plugin", () => {
    assert.equal(manifest.specVersion, "1.0.0");
    assert.equal(manifest.od.kind, "skill");
    assert.match(manifest.version, /^\d+\.\d+\.\d+$/);
    // The registry names the installed folder after `name`, which is also the skill's name.
    assert.equal(manifest.name, "weft");
    assert.equal(frontmatter(text(join(PLUGIN, "SKILL.md")))?.["name"], manifest.name);
  });

  test("asks for exactly what running the bundled scripts takes, and nothing elevated beyond it", () => {
    // `bash` runs `node dist/<script>.js`; `mcp` and `subprocess` are not declared because the
    // manifest does not register the MCP server (see the README).
    assert.deepEqual(manifest.od.capabilities.toSorted(), ["bash", "fs:read", "prompt:inject"]);
    assert.equal("mcp" in manifest.od.context, false);
    // The v1 vocabulary of Open Design's doctor (packages/plugin-runtime/src/validate.ts); an unknown
    // capability is only a warning there, but a typo should not ship.
    const known = [
      "prompt:inject",
      "fs:read",
      "fs:write",
      "mcp",
      "subprocess",
      "bash",
      "network",
      "connector",
    ];
    for (const capability of manifest.od.capabilities)
      assert.ok(known.includes(capability), capability);
  });

  test("points its skill paths at files that exist", () => {
    const paths = [
      ...manifest.compat.agentSkills.map((skill: { path: string }) => skill.path),
      ...manifest.od.context.skills.map((skill: { path: string }) => skill.path),
    ];
    assert.ok(paths.length >= 2);
    for (const path of paths) {
      assert.match(path, /^\.\//);
      assert.equal(resolve(PLUGIN, path), join(PLUGIN, "SKILL.md"));
      assert.ok(existsSync(resolve(PLUGIN, path)), path);
    }
  });

  test("matches the other Weft plugins' identity", () => {
    const other = json(join(ROOT, "plugins/cursor/.cursor-plugin/plugin.json"));
    for (const key of ["version", "license"]) assert.equal(manifest[key], other[key], key);
    assert.equal(manifest.author.name, other.author.name);
  });

  test("is a manifest the schema would refuse when it is wrong", () => {
    const { name: _name, ...nameless } = manifest;
    assert.equal(validateManifest(nameless), false);
    assert.equal(validateManifest({ ...manifest, name: "Weft Design" }), false);
    assert.equal(validateManifest({ ...manifest, od: { ...manifest.od, kind: "plugin" } }), false);
  });
});

describe("SKILL.md", () => {
  const source = text(join(PLUGIN, "SKILL.md"));
  const fields = frontmatter(source);

  test("has the Agent Skills fields: a lowercase name and a description within the limits", () => {
    assert.match(String(fields?.["name"]), /^[a-z0-9]+(-[a-z0-9]+)*$/);
    assert.ok(String(fields?.["name"]).length <= 64);
    const description = String(fields?.["description"]);
    assert.ok(description.length > 20 && description.length <= 1024, `${description.length}`);
  });

  test("keeps its frontmatter in the subset Open Design's own parser reads: flat keys and one nested mapping", () => {
    // Open Design reads SKILL.md with a small hand-written parser, not a YAML library: no anchors,
    // no flow mappings, no folded scalars. Run once against the real parser (see the README).
    const block = /^---\n([\s\S]*?)\n---\n/.exec(source)?.[1] ?? "";
    for (const line of block.split("\n"))
      assert.match(line, /^( {2})?[a-z]+: \S.*$|^metadata:$/, line);
    assert.equal(typeof fields?.["metadata"], "object");
  });

  test("names only scripts that exist, by a path from the Skill root, and uses no host variable", () => {
    const scripts = [...source.matchAll(/<Skill root>\/(dist\/[\w.-]+)/g)].map(
      (m) => m[1] as string,
    );
    assert.deepEqual([...new Set(scripts)].toSorted(), [
      "dist/design-md.js",
      "dist/export.js",
      "dist/import.js",
      "dist/render.js",
    ]);
    for (const script of scripts) assert.ok(existsSync(join(PLUGIN, script)), script);
    assert.doesNotMatch(source, /PLUGIN_ROOT|\$\{/);
  });

  test("links the guide, which is a real copy of the repository's AGENT-SPEC.md", () => {
    assert.match(source, /\]\(references\/AGENT-SPEC\.md\)/);
    const copy = join(PLUGIN, "references/AGENT-SPEC.md");
    assert.equal(lstatSync(copy).isSymbolicLink(), false);
    assert.equal(text(copy), text(join(ROOT, "AGENT-SPEC.md")));
  });

  test("tells the agent to show every loss table and to keep output out of the skill folder", () => {
    assert.match(source, /loss table/);
    assert.match(source, /not into the skill folder/);
  });
});

describe("the plugin folder as `od plugin install` takes it", () => {
  test("holds no symlink, no path that climbs out and less than the installer's 50 MiB cap", () => {
    let bytes = 0;
    for (const file of filesIn(PLUGIN)) {
      assert.ok(!file.split("/").includes(".."), file);
      const stat = lstatSync(join(PLUGIN, file));
      assert.equal(stat.isSymbolicLink(), false, file);
      if (stat.isFile()) bytes += stat.size;
    }
    assert.ok(bytes < 50 * 1024 * 1024, `${bytes} bytes`);
  });

  test("ships the bundle at the paths the skill reads", () => {
    for (const file of ["design-md", "export", "import", "render", "server"]) {
      assert.ok(statSync(join(PLUGIN, "dist", `${file}.js`)).size > 0, file);
    }
    assert.ok(existsSync(join(PLUGIN, "dist/wasm-web/weft_bg.wasm")));
  });
});

describe("the plugin folder alone", () => {
  const scratch = mkdtempSync(join(tmpdir(), "weft-open-design-"));
  // The Skill root as the host states it: a folder inside the project, named after the skill.
  const project = join(scratch, "project");
  const root = join(project, ".od-skills/weft-0123456789ab");
  const corpus = join(ROOT, "corpus");
  // Coverage screens have no hand-written page (corpus/README.md); the import needs one.
  const screen = readdirSync(corpus, { withFileTypes: true }).find(
    (e) => e.isDirectory() && existsSync(join(corpus, e.name, "screen.html")),
  )?.name;
  assert.ok(screen);

  beforeAll(() => {
    installCopy("open-design", root);
  });
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));

  /** Runs a script the way the skill says to: `node "<Skill root>/dist/<script>.js"` from the project. */
  const run = (script: string, ...args: string[]) =>
    spawnSync(process.execPath, [join(root, "dist", `${script}.js`), ...args], {
      cwd: project,
      encoding: "utf8",
    });

  test("import writes the screen and prints the loss table", () => {
    const out = join(project, "imported.weft");
    const result = run("import", join(corpus, screen!, "screen.html"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Import losses:/);
    assert.match(result.stdout, /\| Kind \| Path \| Note \|/);
    assert.ok(text(out).length > 0);
  });

  test("export writes the React component", () => {
    const out = join(project, "Screen.jsx");
    const result = run("export", join(corpus, screen!, "screen.weft"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(text(out), /export default function/);
  });

  test("render writes the page and prints the file URL", () => {
    const out = join(project, "page.html");
    const data = join(corpus, screen!, "data.json");
    const result = run("render", join(corpus, screen!, "screen.weft"), out, "--data", data);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /^file:\/\//m);
    assert.match(text(out), /<html/i);
  });

  test("render validates strictly: a broken screen exits 1 with diagnostics and writes nothing", () => {
    const bad = join(project, "bad.weft");
    const out = join(project, "bad.html");
    writeFileSync(
      bad,
      text(join(corpus, screen!, "screen.weft")).replace(/variant="[^"]+"/, 'variant="nope"'),
    );
    const result = run("render", bad, out);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /bad\.weft:\d+:\d+ /);
    assert.equal(existsSync(out), false);
  });

  test("design-md maps an Open Design design system folder to tokens the loader accepts", () => {
    const systems = join(project, "design-systems");
    mkdirSync(systems, { recursive: true });
    const input = join(systems, "minimal");
    installCopyFixture(join(DESIGN_SYSTEMS, "open-design/minimal"), input);
    const result = run("design-md", input);
    assert.equal(result.status, 0, result.stderr);
    const tokens = json(join(systems, "minimal.tokens.json"));
    assert.deepEqual(loadTokens(tokens).problems, []);
    assert.match(result.stdout, /\| Kind \| Path \| Note \|/);
    assert.match(result.stdout, /--accent-hover/);
  });

  test("design-md reads a DESIGN.md and sends the file where weft.json says", () => {
    const work = join(project, "with-project");
    mkdirSync(work, { recursive: true });
    writeFileSync(
      join(work, "weft.json"),
      JSON.stringify({ plugins: { "open-design": { tokensDir: "tokens" } } }),
    );
    const input = join(work, "DESIGN.md");
    writeFileSync(input, text(join(DESIGN_SYSTEMS, "google-labs/paws-and-paths/DESIGN.md")));
    const result = run("design-md", input);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(loadTokens(json(join(work, "tokens/DESIGN.tokens.json"))).problems, []);
    assert.match(result.stdout, /\| component \|/);
  });

  test("a prose-only DESIGN.md is refused with exit 1 and a pointer to tokens.css", () => {
    const result = run(
      "design-md",
      join(DESIGN_SYSTEMS, "open-design/minimal/DESIGN.md"),
      "--no-project",
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /tokens\.css/);
  });

  test("a script reports a usage problem with exit code 2", () => {
    const result = run("design-md");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /usage/);
  });

  test("the weft MCP server starts from dist/server.js and serves its tools", async () => {
    const session = await startServer([join(root, "dist/server.js")], project);
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

/** Copies a fixture folder, so a script writing beside it does not touch the repository. */
function installCopyFixture(from: string, to: string): void {
  mkdirSync(to, { recursive: true });
  for (const name of readdirSync(from))
    writeFileSync(join(to, name), readFileSync(join(from, name)));
}
