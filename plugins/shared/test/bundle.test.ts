// Each plugin works from its own folder alone, which is all Claude Code and Cursor copy into their
// plugin caches: the committed bundles are current in every plugin, they import nothing but Node
// built-ins, and the scripts and the MCP server run from a copy of each folder placed outside the
// repository.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseAst } from "vite";
import { afterAll, beforeAll, describe, test } from "vitest";
import { buildBundle, distDir, PLUGINS, specCopy, type PluginName } from "../build.ts";
import { main as exportMain } from "../scripts/export.ts";
import { main as importMain } from "../scripts/import.ts";
import { main as renderMain } from "../scripts/render.ts";
import { installCopy } from "./install.ts";
import { startServer, WEFT_TOOLS } from "./mcp-stdio.ts";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const REPOSITORY = resolve(here("../../.."));
const NAMES = Object.keys(PLUGINS) as PluginName[];
const CORPUS = here("../../../corpus");
const screens = readdirSync(CORPUS, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

/** The module specifiers a bundle imports statically or dynamically. */
function importSpecifiers(code: string): string[] {
  const found: string[] = [];
  const visit = (node: unknown): void => {
    if (Array.isArray(node)) return node.forEach(visit);
    if (typeof node !== "object" || node === null) return;
    const { type, source } = node as { type?: string; source?: { value?: unknown } };
    if (
      (type === "ImportDeclaration" ||
        type === "ExportNamedDeclaration" ||
        type === "ExportAllDeclaration" ||
        type === "ImportExpression") &&
      typeof source?.value === "string"
    ) {
      found.push(source.value);
    }
    Object.values(node).forEach(visit);
  };
  visit(parseAst(code));
  return found;
}

/** Every file under `dir` as a path relative to it, sorted. */
const filesIn = (dir: string): string[] =>
  readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => relative(dir, join(entry.parentPath, entry.name)))
    .toSorted();

const jsFiles = (dir: string) => filesIn(dir).filter((file) => file.endsWith(".js"));

let scratch: string;
beforeAll(() => {
  scratch = mkdtempSync(join(tmpdir(), "weft-bundle-"));
});
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

describe("the committed bundles", () => {
  test("are what the sources build to, so a source change needs `moon run shared:build`", async () => {
    const out = join(scratch, "rebuilt");
    await buildBundle(out);
    for (const name of NAMES) {
      const committed = distDir(name);
      assert.deepEqual(filesIn(committed), filesIn(out), name);
      for (const file of filesIn(out)) {
        assert.ok(
          readFileSync(join(committed, file)).equals(readFileSync(join(out, file))),
          `${name}: ${file} is stale`,
        );
      }
      assert.equal(
        readFileSync(specCopy(name), "utf8"),
        readFileSync(join(REPOSITORY, "AGENT-SPEC.md"), "utf8"),
        `${name}: AGENT-SPEC.md copy is stale`,
      );
    }
  });
});

describe.each(NAMES)("the committed bundle of the %s plugin", (name) => {
  const dist = distDir(name);

  test("import nothing but Node built-ins and files of their own folder", () => {
    for (const file of jsFiles(dist)) {
      for (const specifier of importSpecifiers(readFileSync(join(dist, file), "utf8"))) {
        if (specifier.startsWith("node:")) continue;
        assert.match(specifier, /^\.\.?\//, `${file} imports ${specifier}`);
        assert.ok(
          existsSync(resolve(dist, file, "..", specifier)),
          `${file} imports ${specifier}, which does not exist`,
        );
      }
    }
  });

  test("ship the WebAssembly module at every path they read it from", () => {
    // Paths with a folder part are the ones read from disk. The bare `weft_bg.wasm` of the
    // generated glue is its fetch fallback, which Node never takes (the core reads the file).
    const named = new Set<string>();
    for (const file of jsFiles(dist)) {
      const code = readFileSync(join(dist, file), "utf8");
      for (const [, name] of code.matchAll(/["'`](\.\.?\/[^"'`\s]*\.wasm)["'`]/g)) {
        const path = resolve(dist, file, "..", name as string);
        named.add(path);
        assert.ok(existsSync(path), `${file} names ${name}`);
      }
    }
    assert.ok(named.size > 0, "the bundles load the WebAssembly core from a file");
  });

  test("each program says so when Node is too old to tell it was started as one", () => {
    for (const entry of ["import", "export", "render", "design-md", "server"]) {
      const code = readFileSync(join(dist, `${entry}.js`), "utf8");
      assert.match(code, /Node 24\.2 or later is required/, entry);
    }
  });

  test("name no home directory, user or checkout path, so they build the same anywhere", () => {
    for (const file of filesIn(dist)) {
      const text = readFileSync(join(dist, file)).toString("latin1");
      assert.equal(/\/(Users|home)\/[\w.-]+/.test(text), false, file);
    }
  });

  test("do not refer to the repository by a path", () => {
    for (const file of jsFiles(dist)) {
      const code = readFileSync(join(dist, file), "utf8");
      assert.equal(code.includes(REPOSITORY), false, file);
    }
  });
});

describe.each(NAMES)("the %s plugin folder alone", (plugin) => {
  const copy = () => join(scratch, "cache", plugin, "weft/0.1.0");
  /** An empty directory that is neither the plugin nor the repository, like a user's project. */
  const project = () => join(scratch, "project", plugin);

  beforeAll(() => {
    installCopy(plugin, copy());
    mkdirSync(project(), { recursive: true });
  });

  const node = (script: string, ...args: string[]) =>
    spawnSync(process.execPath, [join(copy(), "dist", script), ...args], {
      cwd: project(),
      encoding: "utf8",
    });

  describe.each(screens)("corpus screen %s", (name) => {
    const dir = () => join(project(), name);
    const source = join(CORPUS, name, "screen.weft");
    const data = join(CORPUS, name, "data.json");
    const inProcess = (main: (argv: readonly string[], io: never) => number, ...argv: string[]) => {
      main(argv, { stdout: () => undefined, stderr: () => undefined } as never);
    };

    test("render writes the page the sources write", () => {
      mkdirSync(dir(), { recursive: true });
      const bundled = join(dir(), "bundled.html");
      const sourced = join(dir(), "sourced.html");
      const result = node("render.js", source, bundled, "--data", data);
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^file:\/\//m);
      inProcess(renderMain, source, sourced, "--data", data);
      assert.equal(readFileSync(bundled, "utf8"), readFileSync(sourced, "utf8"));
    });

    test("export writes the component the sources write", () => {
      mkdirSync(dir(), { recursive: true });
      const bundled = join(dir(), "bundled.jsx");
      const sourced = join(dir(), "sourced.jsx");
      const result = node("export.js", source, bundled);
      assert.equal(result.status, 0, result.stderr);
      inProcess(exportMain, source, sourced);
      assert.equal(readFileSync(bundled, "utf8"), readFileSync(sourced, "utf8"));
    });

    // Coverage screens have no hand-written page to import (corpus/README.md).
    test.runIf(existsSync(join(CORPUS, name, "screen.html")))(
      "import writes the screen the sources write",
      () => {
        mkdirSync(dir(), { recursive: true });
        const page = join(CORPUS, name, "screen.html");
        const bundled = join(dir(), "bundled.weft");
        const sourced = join(dir(), "sourced.weft");
        const result = node("import.js", page, bundled);
        assert.equal(result.status, 0, result.stderr);
        inProcess(importMain, page, sourced);
        assert.equal(readFileSync(bundled, "utf8"), readFileSync(sourced, "utf8"));
      },
    );
  });

  test("a script reports a usage problem with exit code 2", () => {
    const result = node("render.js");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /usage/);
  });

  test("render's built-in default tokens are the catalog's default token file", () => {
    const source = join(CORPUS, screens[0] as string, "screen.weft");
    const defaults = join(REPOSITORY, "packages/catalog/tokens/default.tokens.json");
    const built = node("render.js", source, join(project(), "built-in.html"));
    const given = node("render.js", source, join(project(), "given.html"), "--tokens", defaults);
    assert.equal(built.status, 0, built.stderr);
    assert.equal(given.status, 0, given.stderr);
    assert.equal(
      readFileSync(join(project(), "built-in.html"), "utf8"),
      readFileSync(join(project(), "given.html"), "utf8"),
    );
  });

  test("the MCP server serves the weft tools, validates and renders a page", async () => {
    const session = await startServer([join(copy(), "dist/server.js")], project());
    try {
      assert.deepEqual((await session.tools()).toSorted(), WEFT_TOOLS);
      const markup = readFileSync(join(CORPUS, screens[0] as string, "screen.weft"), "utf8");
      const validated = await session.call("weft_validate", { markup });
      assert.notEqual(validated.isError, true, validated.content[0]?.text ?? "");
      const rendered = await session.call("weft_render", { markup, html: true });
      assert.notEqual(rendered.isError, true, rendered.content[0]?.text ?? "");
      assert.match(rendered.content.at(-1)?.text ?? "", /<html/i);
    } finally {
      session.close();
    }
  });
});
