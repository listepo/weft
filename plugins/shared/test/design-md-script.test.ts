// The design-md script: a design system in, DTCG tokens and a loss table out, and where the project
// file sends them. Driven in process with captured output, on the fixtures of `@weft/design-md`.
import assert from "node:assert/strict";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { loadTokens } from "@weft/catalog";
import { afterAll, beforeAll, test } from "vitest";
import { main } from "../scripts/design-md.ts";

const FIXTURES = fileURLToPath(
  new URL("../../../packages/design-md/test/fixtures", import.meta.url),
);

function run(...argv: string[]) {
  const out: string[] = [];
  const err: string[] = [];
  const code = main(argv, { stdout: (t) => out.push(t), stderr: (t) => err.push(t) });
  return { code, stdout: out.join(""), stderr: err.join("") };
}

let dir: string;
let n = 0;
/** A fresh folder holding a copy of both fixture families. */
const work = (): string => {
  const path = join(dir, String((n += 1)));
  cpSync(FIXTURES, path, { recursive: true });
  return path;
};
beforeAll(() => {
  dir = mkdtempSync(join(tmpdir(), "weft-design-md-"));
});
afterAll(() => rmSync(dir, { recursive: true, force: true }));

test("a DESIGN.md becomes a token file next to it, valid for the token loader, with its losses printed", () => {
  const root = work();
  const input = join(root, "google-labs/paws-and-paths/DESIGN.md");
  const result = run(input, "--no-project");
  assert.equal(result.code, 0, result.stderr);
  const output = join(root, "google-labs/paws-and-paths/DESIGN.tokens.json");
  assert.deepEqual(loadTokens(JSON.parse(readFileSync(output, "utf8"))).problems, []);
  assert.match(result.stdout, /^Wrote .*DESIGN\.tokens\.json$/m);
  assert.match(result.stdout, /\| component \| components\.button-primary \|/);
  assert.match(result.stdout, /\| converted \| typography\.display \|/);
});

test("a design system folder is read from its tokens.css, and the output lands beside the folder", () => {
  const root = work();
  const result = run(join(root, "open-design/minimal"), "--no-project");
  assert.equal(result.code, 0, result.stderr);
  assert.match(readFileSync(join(root, "open-design/minimal.tokens.json"), "utf8"), /"elev-ring"/);
  assert.match(result.stdout, /color-mix/);
});

test("an existing output is kept unless --force, and a second output path can be named", () => {
  const root = work();
  const input = join(root, "open-design/minimal/tokens.css");
  const target = join(root, "out.tokens.json");
  assert.equal(run(input, target, "--no-project").code, 0);
  const again = run(input, target, "--no-project");
  assert.equal(again.code, 2);
  assert.match(again.stderr, /--force/);
  assert.equal(run(input, target, "--no-project", "--force").code, 0);
});

test("plugins.open-design.tokensDir in weft.json decides the folder, created when missing", () => {
  const root = work();
  const project = join(root, "project");
  mkdirSync(project);
  writeFileSync(
    join(project, "weft.json"),
    JSON.stringify({ plugins: { "open-design": { tokensDir: "tokens/imported" } } }),
  );
  const result = run(
    join(root, "open-design/minimal/tokens.css"),
    "--project",
    join(project, "weft.json"),
  );
  assert.equal(result.code, 0, result.stderr);
  assert.ok(existsSync(join(project, "tokens/imported/tokens.tokens.json")));
  assert.match(result.stdout, /List it under "tokens" in .*weft\.json/);
});

test("a tokensDir the loader refuses stops the script with the loader's diagnostic", () => {
  const root = work();
  const project = join(root, "project");
  mkdirSync(project);
  for (const tokensDir of ["../escape", "/etc", 3, ""]) {
    writeFileSync(
      join(project, "weft.json"),
      JSON.stringify({ plugins: { "open-design": { tokensDir } } }),
    );
    const result = run(
      join(root, "open-design/minimal/tokens.css"),
      "--project",
      join(project, "weft.json"),
    );
    assert.equal(result.code, 1, JSON.stringify(tokensDir));
    assert.match(result.stderr, /plugins\/open-design\/tokensDir W70[13] /);
  }
  assert.equal(existsSync(join(root, "escape")), false);
  assert.equal(existsSync(join(root, "open-design/minimal/tokens.tokens.json")), false);
});

test("an unknown key in plugins.open-design only warns and the default folder applies", () => {
  const root = work();
  const project = join(root, "project");
  mkdirSync(project);
  writeFileSync(
    join(project, "weft.json"),
    JSON.stringify({ plugins: { "open-design": { tokenDir: "tokens" } } }),
  );
  const input = join(root, "open-design/minimal/tokens.css");
  const result = run(input, "--project", join(project, "weft.json"));
  assert.equal(result.code, 0, result.stderr);
  assert.match(result.stderr, /plugins\/open-design\/tokenDir W702 /);
  assert.ok(existsSync(join(root, "open-design/minimal/tokens.tokens.json")));
  assert.equal(existsSync(join(project, "tokens")), false);
});

test("a file without tokens, a missing path and a bad command line each stop with their own code", () => {
  const root = work();
  const prose = run(join(root, "open-design/minimal/DESIGN.md"), "--no-project");
  assert.equal(prose.code, 1);
  assert.match(prose.stderr, /tokens\.css/);
  assert.equal(run(join(root, "nowhere"), "--no-project").code, 2);
  assert.equal(run("--no-project").code, 2);
  assert.equal(run(join(root, "a"), "b", "c", "--no-project").code, 2);
  assert.equal(run("--bogus").code, 2);
  mkdirSync(join(root, "empty"));
  assert.match(
    run(join(root, "empty"), "--no-project").stderr,
    /neither tokens\.css nor DESIGN\.md/,
  );
});
