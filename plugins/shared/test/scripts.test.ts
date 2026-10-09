// The import, export and render scripts on every corpus screen, and their failure paths. The
// scripts are driven in process through `main`, with captured output, and once as real commands.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { hasErrors, parse, validate } from "@weft/core";
import { main as exportMain } from "../scripts/export.ts";
import { main as importMain } from "../scripts/import.ts";
import { main as renderMain } from "../scripts/render.ts";

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const CORPUS = here("../../../corpus");
const screens = readdirSync(CORPUS, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name);

function run(main: (argv: readonly string[], io: never) => number, ...argv: string[]) {
  const out: string[] = [];
  const err: string[] = [];
  const io = { stdout: (t: string) => out.push(t), stderr: (t: string) => err.push(t) };
  const code = main(argv, io as never);
  return { code, stdout: out.join(""), stderr: err.join("") };
}

let dir: string;
beforeAll(() => {
  dir = mkdtempSync(join(tmpdir(), "weft-plugin-"));
});
afterAll(() => rmSync(dir, { recursive: true, force: true }));

test("the corpus has screens to run on", () => {
  assert.ok(screens.length >= 12, screens.join());
});

describe.each(screens)("corpus screen %s", (name) => {
  const source = join(CORPUS, name, "screen.weft");
  const data = join(CORPUS, name, "data.json");
  const work = () => {
    const path = join(dir, name);
    mkdirSync(path, { recursive: true });
    return path;
  };

  test("export writes a component that exports the screen function", () => {
    const out = join(work(), "screen.jsx");
    const result = run(exportMain, source, out);
    assert.equal(result.code, 0, result.stderr);
    assert.match(
      readFileSync(out, "utf8"),
      /export default function WeftScreen\(\{ data, actions, onChange \}\)/,
    );
  });

  test("render writes a page with the screen's data and prints its file URL", () => {
    const out = join(work(), "screen.html");
    const result = run(renderMain, source, out, "--data", data);
    assert.equal(result.code, 0, result.stderr);
    const html = readFileSync(out, "utf8");
    assert.ok(html.startsWith("<!doctype html>"));
    assert.ok(html.includes(`<title>Weft: screen</title>`));
    assert.match(result.stdout, new RegExp(`file://.*${name}/screen\\.html\\n$`));
  });

  test("the rendered page imports back to a valid screen and a loss table", () => {
    const page = join(work(), "page.html");
    assert.equal(run(renderMain, source, page, "--data", data).code, 0);
    const out = join(work(), "imported.weft");
    const result = run(importMain, page, out);
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /\| Kind \| Path \| Note \|/);
    assert.match(result.stdout, /\| bindings \|/);
    const parsed = parse(readFileSync(out, "utf8"), { catalog: coreCatalog, mode: "lenient" });
    assert.ok(parsed.document !== undefined);
    assert.equal(hasErrors(parsed.diagnostics), false, JSON.stringify(parsed.diagnostics));
    assert.equal(
      hasErrors(validate(parsed.document, { catalog: coreCatalog, mode: "lenient" })),
      false,
    );
  });

  // Coverage screens have no hand-written page to import (corpus/README.md).
  test.runIf(existsSync(join(CORPUS, name, "screen.html")))("the corpus HTML imports", () => {
    const out = join(work(), "from-corpus.weft");
    const result = run(importMain, join(CORPUS, name, "screen.html"), out);
    assert.equal(result.code, 0, result.stderr);
    assert.ok(existsSync(out));
  });
});

describe("generated files land next to their source", () => {
  test("default output paths swap the extension", () => {
    const folder = mkdtempSync(join(dir, "next-"));
    const screen = join(folder, "login.weft");
    writeFileSync(screen, readFileSync(join(CORPUS, "login", "screen.weft")));
    assert.equal(run(exportMain, screen).code, 0);
    assert.equal(run(renderMain, screen).code, 0);
    assert.deepEqual(readdirSync(folder).toSorted(), ["login.html", "login.jsx", "login.weft"]);
  });
});

describe("export options", () => {
  const source = join(CORPUS, "login", "screen.weft");

  test("--name sets the component name", () => {
    const out = join(dir, "named.jsx");
    assert.equal(run(exportMain, source, out, "--name", "LoginScreen").code, 0);
    assert.match(readFileSync(out, "utf8"), /export default function LoginScreen\(/);
  });

  test("a name that is not an identifier is refused and nothing is written", () => {
    const out = join(dir, "bad-name.jsx");
    const result = run(exportMain, source, out, "--name", "not valid");
    assert.equal(result.code, 2);
    assert.match(result.stderr, /componentName/);
    assert.equal(existsSync(out), false);
  });
});

describe("render options", () => {
  const source = join(CORPUS, "login", "screen.weft");

  test("--data feeds the bindings", () => {
    const out = join(dir, "data.html");
    const data = join(dir, "data.json");
    writeFileSync(data, JSON.stringify({ email: "grace@example.com", password: "" }));
    assert.equal(run(renderMain, source, out, "--data", data).code, 0);
    assert.ok(readFileSync(out, "utf8").includes("grace@example.com"));
  });

  test("--tokens replaces the default tokens", () => {
    const withDefault = join(dir, "default-tokens.html");
    const custom = join(dir, "custom-tokens.html");
    const tokens = join(dir, "tokens.json");
    const original = JSON.parse(
      readFileSync(here("../../../packages/catalog/tokens/default.tokens.json"), "utf8"),
    );
    original.space.md.$value = { value: 77, unit: "px" };
    writeFileSync(tokens, JSON.stringify(original));
    assert.equal(run(renderMain, source, withDefault).code, 0);
    assert.equal(run(renderMain, source, custom, "--tokens", tokens).code, 0);
    assert.notEqual(readFileSync(withDefault, "utf8"), readFileSync(custom, "utf8"));
    assert.ok(readFileSync(custom, "utf8").includes("77px"));
  });

  test("a data file that is not JSON is a failure and nothing is written", () => {
    const out = join(dir, "bad-data.html");
    const data = join(dir, "bad-data.json");
    writeFileSync(data, "{ not json");
    const result = run(renderMain, source, out, "--data", data);
    assert.equal(result.code, 2);
    assert.match(result.stderr, /is not JSON/);
    assert.equal(existsSync(out), false);
  });
});

describe("failures", () => {
  const login = join(CORPUS, "login", "screen.weft");
  const scripts = [
    ["import", importMain, join(CORPUS, "login", "screen.html")],
    ["export", exportMain, login],
    ["render", renderMain, login],
  ] as const;

  describe.each(scripts)("%s", (_name, main, input) => {
    test("an existing output is kept unless --force is given", () => {
      const out = join(dir, `keep-${_name}.out`);
      writeFileSync(out, "precious");
      const refused = run(main, input, out);
      assert.equal(refused.code, 2);
      assert.match(refused.stderr, /already exists; pass --force/);
      assert.equal(readFileSync(out, "utf8"), "precious");
      assert.equal(run(main, input, out, "--force").code, 0);
      assert.notEqual(readFileSync(out, "utf8"), "precious");
    });

    test("a missing input is a failure", () => {
      const result = run(main, join(dir, "missing.in"));
      assert.equal(result.code, 2);
      assert.match(result.stderr, /cannot read/);
    });

    test("no arguments, an unknown option and extra arguments print the usage", () => {
      for (const argv of [[], ["--nope"], [input, join(dir, "x"), "extra"]]) {
        const result = run(main, ...argv);
        assert.equal(result.code, 2, argv.join(" "));
        assert.match(result.stderr, /usage: /);
      }
    });

    test("an unwritable output is a failure", () => {
      const result = run(main, input, join(dir, "no-such-folder", "out"));
      assert.equal(result.code, 2);
      assert.match(result.stderr, /cannot write/);
    });
  });

  describe.each([
    ["export", exportMain],
    ["render", renderMain],
  ] as const)("%s of an invalid screen", (name, main) => {
    test("reports the diagnostics, exits 1 and writes nothing", () => {
      const screen = join(dir, `invalid-${name}.weft`);
      writeFileSync(screen, '<screen id="s" weft="0.1" label="x"><nonsense id="n"/></screen>');
      const out = join(dir, `invalid-${name}.out`);
      const result = run(main, screen, out);
      assert.equal(result.code, 1);
      assert.match(result.stderr, /invalid-.*\.weft:1:\d+ \w+ /);
      assert.equal(existsSync(out), false);
    });

    test("a syntax error is reported the same way", () => {
      const screen = join(dir, `syntax-${name}.weft`);
      writeFileSync(screen, "<screen");
      const result = run(main, screen, join(dir, `syntax-${name}.out`));
      assert.equal(result.code, 1);
      assert.ok(result.stderr.length > 0);
    });
  });

  test("a file over the limit is refused before it is parsed", () => {
    const big = join(dir, "big.weft");
    writeFileSync(big, " ".repeat(1_000_000));
    const result = run(exportMain, big, join(dir, "big.jsx"));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /longer than/);
  });

  test("import of text that is not a page still yields a (near-empty) screen or diagnostics, never a crash", () => {
    const html = join(dir, "garbage.html");
    writeFileSync(html, "\u0000<<<>>> not html at all");
    const result = run(importMain, html, join(dir, "garbage.weft"));
    assert.ok(result.code === 0 || result.code === 1, result.stderr);
  });
});

describe("project settings (SPEC §10.6)", () => {
  // A copy of the example project, so generated files stay out of the repository.
  const copy = (settings: Record<string, unknown> = {}) => {
    const root = mkdtempSync(join(dir, "project-"));
    cpSync(here("../../../examples/project"), root, { recursive: true });
    const file = join(root, "weft.json");
    writeFileSync(file, JSON.stringify({ ...JSON.parse(readFileSync(file, "utf8")), ...settings }));
    return { root, cart: join(root, "screens", "cart.weft") };
  };

  // A token layer that sets space.sm, which the cart screen's rows use as their gap.
  const spacing = (root: string, name: string, px: number) =>
    writeFileSync(
      join(root, "tokens", name),
      JSON.stringify({ space: { sm: { $type: "dimension", $value: { value: px, unit: "px" } } } }),
    );

  test("render takes the project's catalog, tokens and render.data", () => {
    const { root, cart } = copy({
      tokens: ["tokens/base.tokens.json", "tokens/brand.tokens.json", "tokens/wide.tokens.json"],
    });
    spacing(root, "wide.tokens.json", 29);
    const result = run(renderMain, cart);
    assert.equal(result.code, 0, result.stderr);
    const html = readFileSync(join(root, "screens", "cart.html"), "utf8");
    assert.ok(html.includes("Linen shirt"), "render.data feeds the bindings");
    assert.ok(html.includes("gap:29px"), "the project's token layers apply");
  });

  test("arguments override the settings", () => {
    const { root, cart } = copy();
    const data = join(root, "other.json");
    writeFileSync(data, JSON.stringify({ cart: { items: [{ name: "Wool scarf" }] } }));
    const out = join(root, "explicit.html");
    assert.equal(run(renderMain, cart, out, "--data", data).code, 0);
    const html = readFileSync(out, "utf8");
    assert.ok(html.includes("Wool scarf") && !html.includes("Linen shirt"));
  });

  test("render.tokens replaces the project's tokens", () => {
    const { root, cart } = copy({
      render: {
        data: "sample.data.json",
        tokens: ["tokens/base.tokens.json", "tokens/wide.tokens.json"],
      },
    });
    spacing(root, "wide.tokens.json", 31);
    const result = run(renderMain, cart);
    assert.equal(result.code, 0, result.stderr);
    const html = readFileSync(join(root, "screens", "cart.html"), "utf8");
    assert.ok(html.includes("gap:31px"));
  });

  test("a render.tokens file that is missing stops the render", () => {
    const { root, cart } = copy({ render: { tokens: ["tokens/none.json"] } });
    const result = run(renderMain, cart);
    assert.equal(result.code, 1);
    assert.match(result.stderr, /weft\.json:#\/render\/tokens\/0 W704/);
    assert.equal(existsSync(join(root, "screens", "cart.html")), false);
  });

  test("render draws one side of the appearance: render.appearance, then --appearance", () => {
    const { root, cart } = copy({ render: { data: "sample.data.json", appearance: "dark" } });
    assert.equal(run(renderMain, cart).code, 0);
    const page = join(root, "screens", "cart.html");
    assert.ok(readFileSync(page, "utf8").includes('<meta name="color-scheme" content="dark"/>'));
    assert.equal(run(renderMain, cart, page, "--force", "--appearance", "light").code, 0);
    assert.ok(readFileSync(page, "utf8").includes('<meta name="color-scheme" content="light"/>'));
    const wrong = run(renderMain, cart, page, "--force", "--appearance", "dusk");
    assert.equal(wrong.code, 2);
    assert.match(wrong.stderr, /--appearance must be "light" or "dark"/);
  });

  test("--tokens takes a resolver, with its problems pointing into it", () => {
    const { root, cart } = copy();
    const out = join(root, "resolved.html");
    const resolver = join(root, "tokens", "theme.resolver.json");
    const result = run(renderMain, cart, out, "--tokens", resolver);
    assert.equal(result.code, 0, result.stderr);
    assert.ok(readFileSync(out, "utf8").includes('content="light"'));
    const broken = join(root, "tokens", "broken.resolver.json");
    writeFileSync(
      broken,
      JSON.stringify({ version: "2025.10", resolutionOrder: [{ $ref: "x.json" }] }),
    );
    const failed = run(renderMain, cart, out, "--force", "--tokens", broken);
    assert.match(failed.stderr, /broken\.resolver\.json:#\/resolutionOrder\/0\/\$ref W704/);
  });

  test("outDir settings place each command's output", () => {
    const { root, cart } = copy({
      render: { outDir: "out/pages" },
      export: { react: { outDir: "out/react" } },
      import: { html: { outDir: "out/screens" } },
    });
    assert.equal(run(renderMain, cart).code, 0);
    assert.equal(run(exportMain, cart).code, 0);
    const page = join(root, "out", "pages", "cart.html");
    assert.ok(existsSync(page));
    assert.ok(existsSync(join(root, "out", "react", "cart.jsx")));
    const imported = run(importMain, page);
    assert.equal(imported.code, 0, imported.stderr);
    assert.ok(existsSync(join(root, "out", "screens", "cart.weft")));
  });

  test("export.react settings make the component TSX that keeps its source", () => {
    const { root, cart } = copy({ export: { react: { typescript: true, source: true } } });
    const result = run(exportMain, cart);
    assert.equal(result.code, 0, result.stderr);
    const tsx = readFileSync(join(root, "screens", "cart.tsx"), "utf8");
    assert.ok(tsx.startsWith("/* weft:source react "), tsx.slice(0, 80));
    assert.match(tsx, /typescript/);
  });

  test("export.react.context decides whether the source comment carries the notes", () => {
    for (const [context, kept] of [
      ["keep", true],
      ["strip", false],
    ] as const) {
      const { root, cart } = copy({ export: { react: { source: true, context } } });
      const entry =
        '<entry id="why" by="human" kind="intent" name="Ivan">Carts stay small.</entry>';
      const text = readFileSync(cart, "utf8");
      writeFileSync(
        cart,
        text.replace(/(<screen[^>]*>\n)/, `$1  <context>\n    ${entry}\n  </context>\n`),
      );
      const result = run(exportMain, cart);
      assert.equal(result.code, 0, result.stderr);
      const jsx = readFileSync(join(root, "screens", "cart.jsx"), "utf8");
      assert.equal(jsx.includes("Carts stay small."), kept, context);
    }
  });

  test("--no-project and --project choose the project", () => {
    const { root, cart } = copy();
    // The screen uses the project's own component, unknown to the core catalog.
    const bare = run(exportMain, cart, join(root, "bare.jsx"), "--no-project");
    assert.equal(bare.code, 1);
    assert.match(bare.stderr, /W401/);
    const elsewhere = join(dir, "elsewhere.weft");
    writeFileSync(elsewhere, readFileSync(cart));
    const named = run(
      exportMain,
      elsewhere,
      join(root, "named.jsx"),
      "--project",
      join(root, "weft.json"),
    );
    assert.equal(named.code, 0, named.stderr);
  });

  test("a project with errors stops every script and writes nothing", () => {
    const { root, cart } = copy({ catalog: "missing.json" });
    for (const main of [renderMain, exportMain]) {
      const result = run(main, cart, join(root, "never.out"));
      assert.equal(result.code, 1);
      assert.match(result.stderr, /weft\.json:#\/catalog W704/);
    }
    const page = join(root, "page.html");
    writeFileSync(page, "<main><h1>Hi</h1></main>");
    assert.equal(run(importMain, page, join(root, "never.weft")).code, 1);
    assert.equal(
      existsSync(join(root, "never.out")) || existsSync(join(root, "never.weft")),
      false,
    );
  });
});

describe("the scripts as commands", () => {
  const command = (script: string, ...args: string[]) =>
    spawnSync(process.execPath, [here(`../scripts/${script}`), ...args], { encoding: "utf8" });

  test.each(["import.ts", "export.ts", "render.ts"])(
    "%s without arguments exits 2 with the usage",
    (script) => {
      const result = command(script);
      assert.equal(result.status, 2);
      assert.match(result.stderr, /usage: /);
    },
  );

  test("render writes a page and prints its URL", () => {
    const out = join(dir, "cli.html");
    const result = command("render.ts", join(CORPUS, "login", "screen.weft"), out);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /^Wrote .*cli\.html\nfile:\/\/\//);
    assert.ok(existsSync(out));
  });
});
