import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DEFAULT_MODELS,
  anthropicProvider,
  countAnthropicTokens,
  mockProvider,
} from "../src/provider.ts";
import { countProxy, measure, ratios, renderTokenTable, spread, totals } from "../src/tokens.ts";

test("proxy tokenizer counts tokens deterministically", () => {
  assert.equal(countProxy(""), 0);
  assert.ok(countProxy("hello world") >= 2);
  assert.ok(countProxy("hello world hello world") > countProxy("hello world"));
  assert.equal(countProxy("<button/>"), countProxy("<button/>"));
});

test("measure covers 12 screens in 4 formats and the totals add up", async () => {
  const table = await measure();
  assert.equal(Object.keys(table.rows).length, 12);
  const t = totals(table, "tokens");
  let weft = 0;
  for (const row of Object.values(table.rows)) {
    assert.ok(row.weft.tokens > 0 && row.a2ui.bytes > row.weft.bytes / 10);
    weft += row.weft.tokens;
  }
  assert.equal(t.weft, weft);
  const r = ratios(table, "tokens", "a2ui");
  const s = spread(r);
  assert.ok(s.min <= s.median && s.median <= s.max);
  assert.match(renderTokenTable(table), /Stop-criterion check/);
  assert.match(renderTokenTable(table), /not measured/);
});

interface Call {
  url: string;
  init: RequestInit;
}

const fakeFetch = (responses: { status: number; body: unknown }[], calls: Call[]): typeof fetch =>
  (async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({ url: String(url), init: init ?? {} });
    const next = responses.shift();
    assert.ok(next, "unexpected extra request");
    return new Response(JSON.stringify(next.body), { status: next.status });
  }) as typeof fetch;

test("anthropic provider posts to the Messages API and reads text and usage", async () => {
  const calls: Call[] = [];
  const provider = anthropicProvider({
    apiKey: "k",
    model: DEFAULT_MODELS[0],
    fetch: fakeFetch(
      [
        {
          status: 200,
          body: {
            content: [
              { type: "text", text: "hi " },
              { type: "text", text: "there" },
            ],
            usage: { input_tokens: 7, output_tokens: 3 },
          },
        },
      ],
      calls,
    ),
  });
  const out = await provider.complete("hello");
  assert.deepEqual(out, { text: "hi there", inputTokens: 7, outputTokens: 3 });
  assert.equal(calls[0]?.url, "https://api.anthropic.com/v1/messages");
  const headers = calls[0]?.init.headers as Record<string, string>;
  assert.equal(headers["x-api-key"], "k");
  assert.equal(headers["anthropic-version"], "2023-06-01");
  const body = JSON.parse(String(calls[0]?.init.body));
  assert.equal(body.model, "claude-sonnet-5-5");
  assert.deepEqual(body.messages, [{ role: "user", content: "hello" }]);
});

test("anthropic provider retries overload and then fails with the response body", async () => {
  const calls: Call[] = [];
  const sleeps: number[] = [];
  const base = { apiKey: "k", model: "m", sleep: async (ms: number) => void sleeps.push(ms) };
  const ok = anthropicProvider({
    ...base,
    fetch: fakeFetch(
      [
        { status: 529, body: {} },
        { status: 200, body: { content: [{ type: "text", text: "x" }] } },
      ],
      calls,
    ),
  });
  assert.equal((await ok.complete("p")).text, "x");
  assert.deepEqual(sleeps, [1000]);
  const bad = anthropicProvider({
    ...base,
    fetch: fakeFetch([{ status: 400, body: { error: "bad" } }], []),
  });
  await assert.rejects(bad.complete("p"), /Anthropic API 400.*bad/);
});

test("anthropic token counting uses count_tokens", async () => {
  const calls: Call[] = [];
  const n = await countAnthropicTokens(
    {
      apiKey: "k",
      model: "m",
      fetch: fakeFetch([{ status: 200, body: { input_tokens: 42 } }], calls),
    },
    "doc",
  );
  assert.equal(n, 42);
  assert.equal(calls[0]?.url, "https://api.anthropic.com/v1/messages/count_tokens");
  await assert.rejects(
    countAnthropicTokens(
      { apiKey: "k", model: "m", fetch: fakeFetch([{ status: 200, body: {} }], []) },
      "doc",
    ),
    /no input_tokens/,
  );
});

test("measure adds Anthropic counts when options are given", async () => {
  const responses = Array.from({ length: 48 }, () => ({ status: 200, body: { input_tokens: 10 } }));
  const table = await measure({ apiKey: "k", model: "m", fetch: fakeFetch(responses, []) });
  assert.equal(table.anthropicModel, "m");
  assert.equal(totals(table, "anthropicTokens").weft, 120);
  assert.match(renderTokenTable(table), /Anthropic count_tokens, m/);
});

test("mock provider is a pure function of the prompt", async () => {
  const p = mockProvider((prompt) => prompt.toUpperCase());
  assert.equal((await p.complete("a")).text, "A");
  assert.equal((await p.complete("a")).text, "A");
});
