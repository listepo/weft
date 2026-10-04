// The server's own settings (SPEC §10.6): set by its host, from `weft-mcp --project` or
// `createServer`, and never by a tool argument.
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { serverFor } from "../src/server.ts";
import { call, connect, connectTo } from "./connect.ts";

const EXAMPLE = fileURLToPath(new URL("../../../examples/project/weft.json", import.meta.url));
const UNKNOWN = '<screen id="s" label="S" weft="0.1"><text id="t" foo="1">a</text></screen>';
const validity = (text: string | undefined) =>
  (JSON.parse(text ?? "null") as { valid: boolean }).valid;

test("createServer limits replace the defaults and show in the tool descriptions", async () => {
  const { client, close } = await connect({}, { limits: { markupChars: 80, diagnostics: 1 } });
  const { tools } = await client.listTools();
  assert.match(tools.find((t) => t.name === "weft_format")?.description ?? "", /limited to 80 /);
  const long = await call(client, "weft_validate", { markup: " ".repeat(81) });
  assert.equal(long.isError, true);
  const listed = await call(client, "weft_validate", { markup: "<screen/>" });
  const result = JSON.parse(listed.blocks[0] ?? "null") as {
    diagnostics: unknown[];
    omitted?: number;
  };
  assert.equal(result.diagnostics.length, 1);
  assert.ok((result.omitted ?? 0) > 0);
  await close();
});

test("the server mode is the default of weft_validate's strict; the argument wins", async () => {
  const lenient = await connect();
  assert.equal(
    validity((await call(lenient.client, "weft_validate", { markup: UNKNOWN })).blocks[0]),
    true,
  );
  await lenient.close();

  const strict = await connect({}, { mode: "strict" });
  assert.equal(
    validity((await call(strict.client, "weft_validate", { markup: UNKNOWN })).blocks[0]),
    false,
  );
  const relaxed = await call(strict.client, "weft_validate", { markup: UNKNOWN, strict: false });
  assert.equal(validity(relaxed.blocks[0]), true);
  await strict.close();
});

test("weft-mcp --project serves the project's resources and settings", async () => {
  const errors: string[] = [];
  const server = serverFor(["--project", EXAMPLE], (text) => errors.push(text));
  assert.notEqual(typeof server, "number");
  assert.deepEqual(errors, []);
  if (typeof server === "number") return;
  const { client, close } = await connectTo(server);
  const catalog = await call(client, "weft_catalog", {});
  assert.match(catalog.blocks[0] ?? "", /^catalog shop /);
  // The example sets validate.mode to strict.
  assert.equal(
    validity((await call(client, "weft_validate", { markup: UNKNOWN })).blocks[0]),
    false,
  );
  await close();
});

test("weft-mcp --project applies mcp.limits; a tool's project argument does not", async () => {
  const dir = mkdtempSync(join(tmpdir(), "weft-mcp-"));
  try {
    writeFileSync(join(dir, "weft.json"), JSON.stringify({ mcp: { limits: { markupChars: 50 } } }));
    const server = serverFor(["--project", join(dir, "weft.json")], () => {});
    if (typeof server === "number") throw new Error(`exit ${server}`);
    const { client, close } = await connectTo(server);
    const long = await call(client, "weft_validate", { markup: " ".repeat(51) });
    assert.equal(long.isError, true);
    const lifted = await call(client, "weft_validate", {
      markup: " ".repeat(51),
      project: { mcp: { limits: { markupChars: 1000 } } },
    });
    assert.equal(lifted.isError, true);
    await close();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("weft-mcp refuses a project with errors and a missing file, and reports why", () => {
  const dir = mkdtempSync(join(tmpdir(), "weft-mcp-"));
  try {
    writeFileSync(
      join(dir, "weft.json"),
      '{"catalog": "missing.json", "mcp": {"limits": {"patches": 0}}}',
    );
    const errors: string[] = [];
    assert.equal(
      serverFor(["--project", join(dir, "weft.json")], (t) => errors.push(t)),
      1,
    );
    assert.match(errors.join(""), /#\/catalog W704/);
    assert.match(errors.join(""), /#\/mcp\/limits\/patches W701/);
    assert.equal(
      serverFor(["--project", join(dir, "none.json")], () => {}),
      2,
    );
    assert.equal(
      serverFor(["--bogus"], () => {}),
      2,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
