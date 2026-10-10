import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse } from "@weft/core";
import { documentSchema } from "@weft/core/document-schema";
import { parseAriaSnapshot } from "@weft/render-react";
import { LIMITS, PRIMER } from "../src/index.ts";
import { call, connect } from "./connect.ts";

const GOOD = `<screen id="s" label="Demo" weft="0.1">
  <button id="go" on-press="demo.go">Go</button>
</screen>
`;

type Diagnostics = { valid?: boolean; diagnostics: { code: string; hint?: string }[] };
const json = (text: string | undefined) => JSON.parse(text ?? "null") as Diagnostics;

test("lists exactly the nine tools, each written for a model", async () => {
  const { client, close } = await connect();
  const { tools } = await client.listTools();
  assert.deepEqual(tools.map((t) => t.name).toSorted(), [
    "weft_capabilities",
    "weft_catalog",
    "weft_context",
    "weft_format",
    "weft_patch",
    "weft_primer",
    "weft_render",
    "weft_schema",
    "weft_validate",
  ]);
  for (const tool of tools) {
    assert.ok((tool.description ?? "").length > 80, tool.name);
    assert.equal(tool.annotations?.readOnlyHint, true, `${tool.name} touches nothing`);
    assert.equal(tool.annotations?.openWorldHint, false);
    assert.equal(tool.inputSchema.type, "object");
  }
  const patch = tools.find((t) => t.name === "weft_patch");
  assert.deepEqual(patch?.inputSchema.required?.toSorted(), ["markup", "patches"]);
  await close();
});

test("weft_primer returns the primer and mentions every tool", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_primer", {});
  assert.equal(result.isError, false);
  assert.equal(result.blocks[0], PRIMER);
  for (const name of [
    "weft_capabilities",
    "weft_catalog",
    "weft_schema",
    "weft_validate",
    "weft_format",
    "weft_patch",
    "weft_render",
    "weft_context",
  ])
    assert.ok(PRIMER.includes(name), name);
  await close();
});

test("weft_schema returns the document schema of the host's catalog, as documentSchema writes it", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_schema", {});
  assert.equal(result.isError, false);
  // Byte for byte: @weft/core pins documentSchema to the Rust generator's snapshot.
  assert.equal(result.blocks[0], documentSchema(coreCatalog));
  const schema = JSON.parse(result.blocks[0] ?? "") as { $defs: Record<string, unknown> };
  assert.ok("button" in schema.$defs);
  await close();
});

test("weft_catalog: compact index, one full component, unknown kind", async () => {
  const { client, close } = await connect();
  const index = await call(client, "weft_catalog", {});
  const lines = (index.blocks[0] ?? "").split("\n");
  assert.equal(lines.length, 1 + Object.keys(coreCatalog.components).length);
  assert.ok(
    lines.includes("button | button | text | " + coreCatalog.components["button"]?.description),
  );
  const fullCatalogSize = JSON.stringify(coreCatalog).length;
  assert.ok((index.blocks[0] ?? "").length < fullCatalogSize / 3, "the index is much smaller");

  const one = await call(client, "weft_catalog", { kind: "button" });
  assert.deepEqual(JSON.parse(one.blocks[0] ?? ""), {
    kind: "button",
    ...coreCatalog.components["button"],
  });

  const bad = await call(client, "weft_catalog", { kind: "buton" });
  assert.equal(bad.isError, true);
  assert.match(bad.blocks[0] ?? "", /did you mean "button"\?/);
  const proto = await call(client, "weft_catalog", { kind: "constructor" });
  assert.equal(proto.isError, true, proto.blocks[0]);
  await close();
});

test("weft_validate: valid, invalid with hints, strict, warnings", async () => {
  const { client, close } = await connect();
  const ok = json((await call(client, "weft_validate", { markup: GOOD })).blocks[0]);
  assert.deepEqual(ok, { valid: true, diagnostics: [] });

  const bad = await call(client, "weft_validate", {
    markup: GOOD.replace("<button", '<button variant="primry"'),
  });
  assert.equal(bad.isError, false, "diagnostics are an answer, not a tool failure");
  const parsed = json(bad.blocks[0]);
  assert.equal(parsed.valid, false);
  assert.equal(parsed.diagnostics[0]?.code, "W203");
  assert.equal(parsed.diagnostics[0]?.hint, 'did you mean "primary"?');

  const unknown = GOOD.replace("</screen>", '<fancy id="f"/></screen>');
  const lenient = json((await call(client, "weft_validate", { markup: unknown })).blocks[0]);
  assert.equal(lenient.valid, true);
  assert.equal(lenient.diagnostics[0]?.code, "W401");
  const strict = json(
    (await call(client, "weft_validate", { markup: unknown, strict: true })).blocks[0],
  );
  assert.equal(strict.valid, false);

  const syntax = json((await call(client, "weft_validate", { markup: "<screen" })).blocks[0]);
  assert.equal(syntax.valid, false);
  await close();
});

test("weft_validate caps a flood of diagnostics", async () => {
  const { client, close } = await connect();
  const many = `<screen id="s" weft="0.1">${'<fancy id="x"/>'.repeat(200)}</screen>`;
  const result = JSON.parse(
    (await call(client, "weft_validate", { markup: many, strict: true })).blocks[0] ?? "",
  ) as { diagnostics: unknown[]; omitted: number };
  assert.equal(result.diagnostics.length, LIMITS.diagnostics);
  assert.ok(result.omitted > 0);
  await close();
});

test("weft_format: canonical output, idempotent, errors, warnings", async () => {
  const { client, close } = await connect();
  const messy = `<screen weft="0.1"   id="s" label="Demo"><!-- c --><button on-press="demo.go" id="go">  Go  </button></screen>`;
  const formatted = await call(client, "weft_format", { markup: messy });
  assert.equal(formatted.isError, false);
  assert.equal(formatted.blocks[0], GOOD);
  const again = await call(client, "weft_format", { markup: formatted.blocks[0] });
  assert.equal(again.blocks[0], GOOD);

  const broken = await call(client, "weft_format", {
    markup: '<screen id="s" weft="0.1"><button></screen>',
  });
  assert.equal(broken.isError, true);
  assert.ok(json(broken.blocks[0]).diagnostics.length > 0);

  const warned = await call(client, "weft_format", {
    markup: GOOD.replace("</screen>", '<fancy id="f"/></screen>'),
  });
  assert.equal(warned.isError, false);
  assert.equal(warned.blocks.length, 2);
  assert.equal(json(warned.blocks[1]).diagnostics[0]?.code, "W401");
  await close();
});

test("weft_patch: success returns canonical markup", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_patch", {
    markup: GOOD,
    patches: [
      { op: "set", id: "go", prop: "variant", value: "primary" },
      { op: "insert", parent: "s", markup: '<button id="b2" on-press="demo.two">Two</button>' },
    ],
  });
  assert.equal(result.isError, false);
  assert.equal(
    result.blocks[0],
    `<screen id="s" label="Demo" weft="0.1">
  <button id="go" variant="primary" on-press="demo.go">Go</button>
  <button id="b2" on-press="demo.two">Two</button>
</screen>
`,
  );
  await close();
});

test("weft_patch: failures return diagnostics and apply nothing", async () => {
  const { client, close } = await connect();
  const missing = await call(client, "weft_patch", {
    markup: GOOD,
    patches: [{ op: "remove", id: "gooo" }],
  });
  assert.equal(missing.isError, true);
  const d = json(missing.blocks[0]).diagnostics[0];
  assert.equal(d?.code, "W502");
  assert.equal(d?.hint, 'did you mean "go"?');

  const malformed = await call(client, "weft_patch", {
    markup: GOOD,
    patches: [{ op: "set", id: "go" }],
  });
  assert.equal(malformed.isError, true);
  assert.equal(json(malformed.blocks[0]).diagnostics[0]?.code, "W501");

  const badMarkup = await call(client, "weft_patch", { markup: "<screen", patches: [] });
  assert.equal(badMarkup.isError, true);
  assert.ok(json(badMarkup.blocks[0]).diagnostics.length > 0);

  // The result is validated strictly, as a writer's output must be.
  const strict = await call(client, "weft_patch", {
    markup: GOOD,
    patches: [{ op: "insert", parent: "s", markup: '<fancy id="f"/>' }],
  });
  assert.equal(strict.isError, true);
  assert.equal(json(strict.blocks[0]).diagnostics[0]?.code, "W401");
  await close();
});

const SCREEN = `<screen id="s" label="Inbox" weft="0.1">
  <heading id="h" level="1" text="{$.title}"/>
  <list id="l" label="Messages">
    <each id="e" as="m" in="{$.messages}">
      <item id="m" text="{$m.subject}"/>
    </each>
    <slot name="empty">
      <text id="none">No messages</text>
    </slot>
  </list>
  <form id="f" on-submit="mail.send">
    <field id="to" label="To" type="email" value="{$.to}"/>
    <button id="send" disabled="{!$.to}" submit="true" variant="primary">Send</button>
  </form>
</screen>
`;

test("weft_render: the accessibility tree as aria-snapshot YAML, with data", async () => {
  const { client, close } = await connect();
  const full = await call(client, "weft_render", {
    markup: SCREEN,
    data: { title: "Inbox", messages: [{ subject: "Hi" }, { subject: "Lunch?" }], to: "" },
  });
  assert.equal(full.isError, false);
  assert.equal(full.blocks.length, 1);
  assert.equal(
    full.blocks[0],
    [
      '- main "Inbox":',
      '  - heading "Inbox" [level=1]',
      '  - list "Messages":',
      "    - listitem: Hi",
      "    - listitem: Lunch?",
      '  - textbox "To"',
      '  - button "Send" [disabled]',
    ].join("\n"),
  );
  // The parser of the browser test reads the output back, so it is the same YAML Playwright prints.
  assert.equal(parseAriaSnapshot(full.blocks[0] ?? "").children?.[0]?.role, "main");

  const empty = await call(client, "weft_render", { markup: SCREEN });
  assert.match(empty.blocks[0] ?? "", /- list "Messages": No messages/);
  await close();
});

test("weft_render: html on request, strict validation, errors as results", async () => {
  const { client, close } = await connect();
  const withHtml = await call(client, "weft_render", { markup: GOOD, html: true });
  assert.equal(withHtml.isError, false);
  assert.equal(withHtml.blocks[0], '- main "Demo":\n  - button "Go"');
  assert.match(withHtml.blocks[1] ?? "", /^<!doctype html>\n<html lang="en">/);
  assert.match(withHtml.blocks[1] ?? "", /<button data-weft-id="go" type="button">Go<\/button>/);

  const unknown = await call(client, "weft_render", {
    markup: GOOD.replace("</screen>", '<fancy id="f"/></screen>'),
  });
  assert.equal(unknown.isError, true, "rendering is for valid markup, so it is strict");
  assert.equal(json(unknown.blocks[0]).diagnostics[0]?.code, "W401");

  const broken = await call(client, "weft_render", { markup: "<screen" });
  assert.equal(broken.isError, true);

  const hidden = await call(client, "weft_render", {
    markup: '<screen id="s" hidden="true" weft="0.1"/>',
  });
  assert.equal(hidden.isError, false);
  assert.match(hidden.blocks[0] ?? "", /nothing is exposed/);

  // Data is untrusted: a hostile shape renders as missing values, never throws.
  const hostile = await call(client, "weft_render", {
    markup: SCREEN,
    data: { title: { toString: 1 }, messages: "not a list", constructor: "x" },
  });
  assert.equal(hostile.isError, false);
  await close();
});

test("tool inputs are schema-validated and size-limited; nothing throws", async () => {
  const { client, close } = await connect();
  const cases: [string, Record<string, unknown>][] = [
    ["weft_validate", {}],
    ["weft_validate", { markup: 5 }],
    ["weft_validate", { markup: GOOD, strict: "yes" }],
    ["weft_format", { markup: "x".repeat(LIMITS.markupChars + 1) }],
    ["weft_patch", { markup: GOOD }],
    ["weft_patch", { markup: GOOD, patches: "remove" }],
    [
      "weft_patch",
      {
        markup: GOOD,
        patches: Array.from({ length: LIMITS.patches + 1 }, () => ({ op: "remove", id: "go" })),
      },
    ],
    [
      "weft_patch",
      {
        markup: GOOD,
        patches: [{ op: "insert", parent: "s", markup: "<text id='a'/>".repeat(20_000) }],
      },
    ],
    ["weft_render", { markup: GOOD, html: "yes" }],
    ["weft_render", { markup: GOOD, data: { big: "x".repeat(LIMITS.dataChars) } }],
    ["weft_catalog", { kind: 7 }],
    ["weft_catalog", { kind: "x".repeat(101) }],
    ["weft_schema", { project: 7 }],
    ["weft_nonexistent", {}],
  ];
  for (const [name, args] of cases) {
    const result = await call(client, name, args);
    assert.equal(result.isError, true, `${name} ${JSON.stringify(args).slice(0, 60)}`);
  }
  // At the limit still works.
  const edge = await call(client, "weft_validate", { markup: " ".repeat(LIMITS.markupChars) });
  assert.equal(edge.isError, false);
  await close();
});

test("the server does not read files or the network: a bad tool call cannot name a path", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_validate", { markup: GOOD, path: "/etc/passwd" });
  assert.equal(result.isError, false);
  assert.equal(json(result.blocks[0]).valid, true);
  await close();
});

test("a primer example is valid markup", () => {
  const example = '<button id="b">Go</button>';
  const { diagnostics } = parse(`<screen id="s" weft="0.1">${example}</screen>`, {
    catalog: coreCatalog,
    mode: "strict",
  });
  assert.deepEqual(diagnostics, []);
});
