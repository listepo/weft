import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { main } from "../src/cli.ts";
import { catalogJsonSchema, documentJsonSchema, stringify, parse } from "../src/index.ts";
import { catalog } from "./catalog.ts";

const dir = mkdtempSync(join(tmpdir(), "weft-cli-"));
after(() => rmSync(dir, { recursive: true, force: true }));

const catalogPath = join(dir, "catalog.json");
writeFileSync(catalogPath, JSON.stringify(catalog));

function file(name: string, content: string): string {
  const path = join(dir, name);
  writeFileSync(path, content);
  return path;
}

function run(...argv: string[]) {
  let stdout = "";
  let stderr = "";
  const code = main(argv, { stdout: (t) => (stdout += t), stderr: (t) => (stderr += t) });
  return { code, stdout, stderr };
}

const messy =
  '<screen   weft="0.1" id="s"><!-- note --><button on-press="go" id="b">  Go  </button></screen>';
const canonical =
  '<screen id="s" weft="0.1">\n  <button id="b" on-press="go">Go</button>\n</screen>\n';

test("validate prints one line per diagnostic and exits 1 on errors", () => {
  const path = file(
    "bad.weft",
    '<screen id="s" weft="0.1">\n  <button id="b" variant="primay">x</button>\n</screen>',
  );
  const { code, stdout } = run("validate", path, "--catalog", catalogPath);
  assert.equal(code, 1);
  assert.equal(
    stdout,
    `${path}:2:18 W203 "primay" is not an allowed value. — did you mean "primary"?\n`,
  );
});

test("validate --strict turns unknown content into errors", () => {
  const path = file("unknown.weft", '<screen id="s" weft="0.1"><fancy id="f"/></screen>');
  assert.equal(run("validate", path, "--catalog", catalogPath).code, 0);
  assert.equal(run("validate", path, "--catalog", catalogPath, "--strict").code, 1);
});

test("validate accepts canonical JSON and reports paths for it", () => {
  const json = stringify({
    weft: "0.1",
    root: { kind: "screen", id: "s", children: [{ kind: "text" }] },
  });
  const path = file("doc.weft.json", json);
  const { code, stdout } = run("validate", path, "--catalog", catalogPath);
  assert.equal(code, 1);
  assert.match(stdout, /doc\.weft\.json:\/screen#s\/text\[0\] W202 /);
});

test("validate without a catalog checks syntax only", () => {
  const path = file("syntax.weft", "<screen id='s'/>");
  const { code, stdout, stderr } = run("validate", path);
  assert.equal(code, 1);
  assert.match(stdout, /:1:12 W106 /);
  assert.match(stderr, /no --catalog/);
});

test("fmt prints canonical markup and --write rewrites the file", () => {
  const path = file("messy.weft", messy);
  const printed = run("fmt", path);
  assert.equal(printed.code, 0);
  assert.equal(printed.stdout, canonical);
  assert.equal(run("fmt", path, "--write").code, 0);
  assert.equal(readFileSync(path, "utf8"), canonical);
});

test("fmt refuses markup with syntax errors", () => {
  const path = file("broken.weft", "<screen id=s>");
  const { code, stdout } = run("fmt", path);
  assert.equal(code, 1);
  assert.match(stdout, /W106/);
});

test("usage errors exit 2", () => {
  assert.equal(run().code, 2);
  assert.equal(run("lint", "x").code, 2);
  assert.equal(run("validate", join(dir, "missing.weft")).code, 2);
  assert.equal(
    run("validate", file("ok.weft", canonical), "--catalog", file("bad.json", "{}")).code,
    2,
  );
  assert.equal(run("fmt", "x", "--bogus").code, 2);
});

test("JSON Schemas are generated from the model", () => {
  const documentSchema = documentJsonSchema();
  assert.equal(documentSchema.type, "object");
  assert.deepEqual(documentSchema.required, ["weft", "root"]);
  assert.equal(catalogJsonSchema().type, "object");
  assert.ok(parse(canonical).document);
});
