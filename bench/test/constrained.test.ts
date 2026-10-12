import assert from "node:assert/strict";
import { parse, stringify } from "@weft/core";
import { coreCatalog } from "@weft/catalog";
import { describe, test } from "vitest";
import { SCREENS, loadTasks, readScreen, type EditTask } from "../src/corpus.ts";
import { parsers, schemaFor } from "../src/formats.ts";
import { JSON_FORMATS, type Format } from "../src/neutral.ts";
import { PRIMERS } from "../src/primers.ts";
import {
  anthropicProvider,
  batchingProvider,
  mockProvider,
  openAiProvider,
  type CompleteOptions,
} from "../src/provider.ts";
import { checkEdit, runTask } from "../src/run-tasks.ts";
import { anthropicSchemaProblems } from "../src/schema-limits.ts";

const task = loadTasks().find((t) => t.id === "login.e2") as EditTask;
/** The markup solution of `login.e2`, as canonical JSON: what a correct JSON reply looks like. */
const markupFix = readScreen("login", "weft").replace("{!$.email}", "{$.busy}");
const jsonFix = stringify(parse(markupFix, { catalog: coreCatalog }).document!);
const fenced = (lang: string, doc: string) => `\`\`\`${lang}\n${doc}\n\`\`\``;

describe("derived corpus JSON", () => {
  test("every screen reads into the same neutral tree as its markup", () => {
    for (const screen of SCREENS) {
      const markup = parsers.weft(readScreen(screen, "weft"));
      for (const f of JSON_FORMATS) {
        const json = parsers[f](readScreen(screen, f));
        assert.deepEqual(json.errors, [], `${screen} ${f}`);
        assert.deepEqual(json.tree, markup.tree, `${screen} ${f}`);
      }
    }
  });
  test("both JSON formats get the same primer, which keeps the Weft vocabulary", () => {
    assert.equal(PRIMERS["weft-json"], PRIMERS["weft-json-constrained"]);
    assert.ok(PRIMERS["weft-json"].endsWith(PRIMERS.weft.slice(PRIMERS.weft.indexOf("Universal"))));
  });
});

describe("scoring", () => {
  test("a reply scores the same in JSON as in markup", () => {
    const cases: [string, string][] = [
      [fenced("xml", markupFix), fenced("json", jsonFix)],
      [readScreen("login", "weft"), readScreen("login", "weft-json")],
    ];
    for (const [markup, json] of cases) {
      const expected = checkEdit(task, "weft", markup);
      for (const f of JSON_FORMATS) {
        const got = checkEdit(task, f, json);
        assert.deepEqual(
          [got.valid, got.success, got.failed],
          [expected.valid, expected.success, expected.failed],
        );
      }
    }
    assert.equal(checkEdit(task, "weft-json", jsonFix).success, true);
  });
  test("invalid JSON and documents the validator rejects are invalid", () => {
    const notJson = checkEdit(task, "weft-json", "{");
    assert.equal(notJson.valid, false);
    assert.match(notJson.errors[0] ?? "", /^not JSON/);
    const doc = JSON.parse(jsonFix) as { root: { kind: string } };
    doc.root.kind = "page";
    const unknown = checkEdit(task, "weft-json", JSON.stringify(doc));
    assert.equal(unknown.valid, false);
    assert.ok(unknown.errors.length > 0);
    const broken = checkEdit(
      task,
      "weft-json",
      JSON.stringify({ weft: "0.1", root: { kind: "screen", id: "s", children: "x" } }),
    );
    assert.equal(broken.valid, false);
  });
});

describe("the schema is sent only for the constrained format", () => {
  const run = async (format: Format, replies: string[]) => {
    const seen: (object | undefined)[] = [];
    let n = 0;
    const provider = mockProvider((_prompt, options?: CompleteOptions) => {
      seen.push(options?.schema);
      return replies[n++] ?? "";
    });
    return { result: await runTask(task, format, provider, "mock"), seen };
  };
  test("constrained: on the first and the repair prompt, and the reply is scored as any other", async () => {
    const { result, seen } = await run("weft-json-constrained", ["not json", jsonFix]);
    assert.equal(seen.length, 2);
    for (const s of seen) assert.equal(s, schemaFor("weft-json-constrained"));
    assert.equal(
      (seen[0] as { $schema?: string }).$schema,
      "https://json-schema.org/draft/2020-12/schema",
    );
    assert.deepEqual(
      [result.valid, result.repaired, result.validAfterRepair, result.successAfterRepair],
      [false, true, true, true],
    );
  });
  test("free JSON and markup: never", async () => {
    for (const [format, reply] of [
      ["weft-json", jsonFix],
      ["weft", fenced("xml", markupFix)],
    ] as const) {
      const { result, seen } = await run(format, [reply]);
      assert.deepEqual(seen, [undefined], format);
      assert.equal(result.success, true, format);
    }
  });
  test("a question is answered without a schema", async () => {
    const q = loadTasks().find((t) => t.type === "question")!;
    const seen: unknown[] = [];
    await runTask(
      q,
      "weft-json-constrained",
      mockProvider((_p, o) => (seen.push(o?.schema), "ANSWER: x")),
      "m",
    );
    assert.deepEqual(seen, [undefined]);
  });
});

describe("provider requests", () => {
  const schema = { type: "object", additionalProperties: false };
  const capture = () => {
    const bodies: Record<string, unknown>[] = [];
    const fetch = (async (_url: string, init: RequestInit) => {
      bodies.push(JSON.parse(String(init.body)) as Record<string, unknown>);
      return new Response(
        JSON.stringify({
          content: [{ type: "text", text: "{}" }],
          choices: [{ message: { content: "{}" } }],
        }),
      );
    }) as typeof globalThis.fetch;
    return { bodies, fetch };
  };
  test("Anthropic sends output_config.format", async () => {
    const { bodies, fetch } = capture();
    const p = anthropicProvider({ apiKey: "k", model: "m", fetch });
    await p.complete("q", { schema });
    await p.complete("q");
    assert.deepEqual(bodies[0]?.output_config, { format: { type: "json_schema", schema } });
    assert.equal("output_config" in (bodies[1] ?? {}), false);
  });
  test("OpenAI-compatible servers send a strict response_format", async () => {
    const { bodies, fetch } = capture();
    const p = openAiProvider({ baseUrl: "http://h/v1", model: "m", fetch });
    await p.complete("q", { schema });
    await p.complete("q");
    assert.deepEqual(bodies[0]?.response_format, {
      type: "json_schema",
      json_schema: { name: "document", strict: true, schema },
    });
    assert.equal("response_format" in (bodies[1] ?? {}), false);
  });
  test("batch mode refuses a schema instead of dropping it", async () => {
    const p = batchingProvider(async () => [{ text: "x" }]);
    await assert.rejects(p.complete("q", { schema }), /no schema/);
  });
});

describe("Anthropic limits", () => {
  test("the core document schema is unsupported, with every reason", () => {
    const problems = anthropicSchemaProblems(schemaFor("weft-json-constrained")!);
    assert.equal(problems.length, 4);
    assert.match(problems.join("\n"), /recursive[\s\S]*numerical[\s\S]*16 union[\s\S]*24 optional/);
  });
  test("a small closed schema passes", () => {
    const ok = {
      type: "object",
      properties: { a: { type: "string" }, b: { $ref: "#/$defs/B" } },
      required: ["a", "b"],
      additionalProperties: false,
      $defs: { B: { anyOf: [{ type: "string" }, { type: "null" }] } },
    };
    assert.deepEqual(anthropicSchemaProblems(ok), []);
  });
});
