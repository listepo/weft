import assert from "node:assert/strict";
import { test } from "node:test";
import { loadTasks, type EditTask } from "../src/corpus.ts";
import { anthropicBatch, batchingProvider, openAiProvider } from "../src/provider.ts";
import { planJobs, runJobs } from "../src/run-tasks.ts";

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

test("openai provider returns the answer without reasoning and counts visible tokens", async () => {
  const calls: { url: string; init: RequestInit }[] = [];
  const p = openAiProvider({
    baseUrl: "http://localhost:1234/v1/",
    model: "prism-ml/bonsai-27b",
    fetch: (async (url: string, init: RequestInit) => {
      calls.push({ url, init });
      return json({
        choices: [{ message: { content: "<think>```xml\n<x/>\n```</think>\nANSWER: 2" } }],
        usage: {
          prompt_tokens: 40,
          completion_tokens: 159,
          completion_tokens_details: { reasoning_tokens: 151 },
        },
      });
    }) as typeof fetch,
  });
  const c = await p.complete("q");
  assert.deepEqual(c, { text: "ANSWER: 2", inputTokens: 40, outputTokens: 8 });
  assert.equal(calls[0]?.url, "http://localhost:1234/v1/chat/completions");
  const headers = calls[0]?.init.headers as Record<string, string>;
  assert.equal(headers.authorization, undefined);
  const body = JSON.parse(String(calls[0]?.init.body)) as { model: string; messages: unknown[] };
  assert.equal(body.model, "prism-ml/bonsai-27b");
  assert.deepEqual(body.messages, [{ role: "user", content: "q" }]);
});

test("openai provider sends a key when given and labels its errors", async () => {
  let auth: string | undefined;
  const p = openAiProvider({
    baseUrl: "http://h/v1",
    model: "m",
    apiKey: "sk",
    sleep: async () => {},
    fetch: (async (_url: string, init: RequestInit) => {
      auth = (init.headers as Record<string, string>).authorization;
      return new Response("no such model", { status: 404 });
    }) as typeof fetch,
  });
  await assert.rejects(p.complete("q"), /OpenAI-compatible API 404: no such model/);
  assert.equal(auth, "Bearer sk");
});

/** A fake Message Batches API that answers each prompt with `reply(prompt)`. */
function fakeBatches(reply: (prompt: string) => string) {
  const batches: string[][] = [];
  let polls = 0;
  const fetchFn = (async (url: string, init: RequestInit) => {
    if (url.endsWith("/messages/batches") && init.method === "POST") {
      const { requests } = JSON.parse(String(init.body)) as {
        requests: { custom_id: string; params: { messages: { content: string }[] } }[];
      };
      batches.push(requests.map((r) => r.params.messages[0]?.content ?? ""));
      return json({ id: `b${batches.length - 1}`, processing_status: "in_progress" });
    }
    const status = url.match(/\/messages\/batches\/(b\d+)$/);
    if (status) {
      // Report the batch as still running once, so polling is exercised.
      polls++;
      return polls % 2 === 1
        ? json({ processing_status: "in_progress" })
        : json({ processing_status: "ended", results_url: `https://results/${status[1]}` });
    }
    const results = url.match(/^https:\/\/results\/b(\d+)$/);
    if (results) {
      const prompts = batches[Number(results[1])] ?? [];
      // Lines arrive in any order; custom_id maps them back.
      const lines = prompts
        .map((p, i) =>
          JSON.stringify({
            custom_id: `r${i}`,
            result: {
              type: "succeeded",
              message: { content: [{ type: "text", text: reply(p) }], usage: { output_tokens: 3 } },
            },
          }),
        )
        .reverse();
      return new Response(lines.join("\n") + "\n");
    }
    return new Response("unexpected", { status: 500 });
  }) as typeof fetch;
  return { batches, fetchFn };
}

test("anthropicBatch submits all prompts, polls and maps results back by id", async () => {
  const { batches, fetchFn } = fakeBatches((p) => `echo ${p}`);
  const out = await anthropicBatch(
    { apiKey: "k", model: "m", fetch: fetchFn, sleep: async () => {} },
    ["a", "b", "c"],
  );
  assert.deepEqual(batches, [["a", "b", "c"]]);
  assert.deepEqual(
    out.map((c) => c.text),
    ["echo a", "echo b", "echo c"],
  );
});

test("a failed batch request is an error, not a wrong answer", async () => {
  const fetchFn = (async (url: string, init: RequestInit) => {
    if (init.method === "POST") return json({ id: "b0" });
    if (url.endsWith("/b0")) return json({ processing_status: "ended", results_url: "https://r" });
    return new Response(
      JSON.stringify({
        custom_id: "r0",
        result: { type: "errored", error: { type: "overloaded" } },
      }),
    );
  }) as typeof fetch;
  await assert.rejects(
    anthropicBatch({ apiKey: "k", model: "m", fetch: fetchFn, sleep: async () => {} }, ["a"]),
    /r0 errored.*overloaded/,
  );
});

test("batch mode sends first replies as one batch and repairs as a second", async () => {
  const edit = loadTasks().find((t) => t.id === "login.e2") as EditTask;
  const question = loadTasks().find((t) => t.id === "login.q2");
  assert.ok(question);
  // Every edit reply is invalid, so each one gets exactly one repair prompt.
  const { batches, fetchFn } = fakeBatches((p) =>
    p.includes("Question:") ? "ANSWER: 2" : "```xml\n<screen\n```",
  );
  const provider = batchingProvider((prompts) =>
    anthropicBatch({ apiKey: "k", model: "m", fetch: fetchFn, sleep: async () => {} }, prompts),
  );
  const jobs = planJobs([edit, question], ["weft", "html"], ["m"], 2);
  const { results, error } = await runJobs(jobs, () => provider, Number.MAX_SAFE_INTEGER);
  assert.equal(error, undefined);
  assert.equal(results.length, 8);
  assert.deepEqual(
    batches.map((b) => b.length),
    [8, 4],
  );
  assert.ok(batches[1]?.every((p) => p.includes("Your previous reply")));
  assert.ok(
    results.filter((r) => r.type === "edit").every((r) => r.repaired && !r.validAfterRepair),
  );
});
