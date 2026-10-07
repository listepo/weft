import { Agent, fetch as undiciFetch } from "undici";

export interface Completion {
  text: string;
  inputTokens?: number;
  outputTokens?: number;
}

export interface Provider {
  complete(prompt: string): Promise<Completion>;
}

export const DEFAULT_MODELS = [
  "claude-sonnet-5-5",
  "claude-opus-5-5",
  "claude-haiku-4-5-20251001",
] as const;

const API = "https://api.anthropic.com/v1";
const VERSION = "2023-06-01";

/** LM Studio's default OpenAI-compatible endpoint. */
export const LMSTUDIO_URL = "http://localhost:1234/v1";

interface HttpOptions {
  fetch?: typeof fetch;
  /** Replaced in tests so retries and polling do not wait. */
  sleep?: (ms: number) => Promise<void>;
}

export interface AnthropicOptions extends HttpOptions {
  apiKey: string;
  model: string;
  maxTokens?: number;
}

export interface OpenAiOptions extends HttpOptions {
  baseUrl: string;
  model: string;
  /** Local servers such as LM Studio need none. */
  apiKey?: string;
  /** Reasoning models spend part of it before the answer, so the default is generous. */
  maxTokens?: number;
}

const anthropicHeaders = (apiKey: string) => ({
  "content-type": "application/json",
  "x-api-key": apiKey,
  "anthropic-version": VERSION,
});

const RETRYABLE = new Set([429, 500, 502, 503, 529]);

const realSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

async function request(
  label: string,
  url: string,
  init: { method: "GET" | "POST"; headers: Record<string, string>; body?: unknown },
  o: HttpOptions,
): Promise<Response> {
  const doFetch = o.fetch ?? fetch;
  const sleep = o.sleep ?? realSleep;
  for (let attempt = 0; ; attempt++) {
    const res = await doFetch(url, {
      method: init.method,
      headers: init.headers,
      ...(init.body === undefined ? {} : { body: JSON.stringify(init.body) }),
    });
    if (res.ok) return res;
    if (attempt < 3 && RETRYABLE.has(res.status)) {
      await sleep(1000 * 2 ** attempt);
      continue;
    }
    throw new Error(`${label} ${res.status}: ${(await res.text()).slice(0, 500)}`);
  }
}

const post = async (
  label: string,
  url: string,
  headers: Record<string, string>,
  body: unknown,
  o: HttpOptions,
) => (await request(label, url, { method: "POST", headers, body }, o)).json() as Promise<unknown>;

interface AnthropicMessage {
  content?: { type: string; text?: string }[];
  usage?: { input_tokens?: number; output_tokens?: number };
}

function fromAnthropic(json: AnthropicMessage): Completion {
  const text = (json.content ?? [])
    .filter((b) => b.type === "text")
    .map((b) => b.text ?? "")
    .join("");
  return {
    text,
    ...(json.usage?.input_tokens !== undefined ? { inputTokens: json.usage.input_tokens } : {}),
    ...(json.usage?.output_tokens !== undefined ? { outputTokens: json.usage.output_tokens } : {}),
  };
}

const anthropicParams = (o: AnthropicOptions, prompt: string) => ({
  model: o.model,
  max_tokens: o.maxTokens ?? 8192,
  messages: [{ role: "user", content: prompt }],
});

export function anthropicProvider(o: AnthropicOptions): Provider {
  return {
    async complete(prompt) {
      return fromAnthropic(
        (await post(
          "Anthropic API",
          `${API}/messages`,
          anthropicHeaders(o.apiKey),
          anthropicParams(o, prompt),
          o,
        )) as AnthropicMessage,
      );
    },
  };
}

/**
 * Some local reasoning models inline their thinking in `content`; it is not part of the answer
 * and could hold code blocks that the checkers would mistake for the reply.
 */
const stripThinking = (s: string): string => s.replace(/^\s*<think>[\s\S]*?<\/think>\s*/, "");

/**
 * A local server answers only when the whole reply is ready, which for a reasoning model under
 * parallel load can take longer than the 300 s undici allows for headers by default.
 */
const patient = new Agent({ headersTimeout: 0, bodyTimeout: 0 });
const patientFetch = ((url: string, init?: RequestInit) =>
  undiciFetch(url, { ...(init as object), dispatcher: patient })) as unknown as typeof fetch;

export function openAiProvider(o: OpenAiOptions): Provider {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (o.apiKey) headers.authorization = `Bearer ${o.apiKey}`;
  return {
    async complete(prompt) {
      const json = (await post(
        "OpenAI-compatible API",
        `${o.baseUrl.replace(/\/$/, "")}/chat/completions`,
        headers,
        {
          model: o.model,
          max_tokens: o.maxTokens ?? 32768,
          messages: [{ role: "user", content: prompt }],
        },
        { ...o, fetch: o.fetch ?? patientFetch },
      )) as {
        choices?: { message?: { content?: string | null } }[];
        usage?: {
          prompt_tokens?: number;
          completion_tokens?: number;
          completion_tokens_details?: { reasoning_tokens?: number };
        };
      };
      const u = json.usage;
      // Reasoning is excluded so output size compares the documents the formats produce.
      const visible =
        u?.completion_tokens === undefined
          ? undefined
          : u.completion_tokens - (u.completion_tokens_details?.reasoning_tokens ?? 0);
      return {
        text: stripThinking(json.choices?.[0]?.message?.content ?? ""),
        ...(u?.prompt_tokens !== undefined ? { inputTokens: u.prompt_tokens } : {}),
        ...(visible !== undefined ? { outputTokens: visible } : {}),
      };
    },
  };
}

/** Sends all prompts as one Message Batch (half the price, asynchronous) and waits for it. */
export async function anthropicBatch(
  o: AnthropicOptions & { pollMs?: number },
  prompts: string[],
): Promise<Completion[]> {
  const headers = anthropicHeaders(o.apiKey);
  const created = (await post(
    "Anthropic API",
    `${API}/messages/batches`,
    headers,
    { requests: prompts.map((p, i) => ({ custom_id: `r${i}`, params: anthropicParams(o, p) })) },
    o,
  )) as { id: string };
  const sleep = o.sleep ?? realSleep;
  let batch: { processing_status?: string; results_url?: string | null };
  for (;;) {
    batch = (await (
      await request(
        "Anthropic API",
        `${API}/messages/batches/${created.id}`,
        { method: "GET", headers },
        o,
      )
    ).json()) as typeof batch;
    if (batch.processing_status === "ended") break;
    await sleep(o.pollMs ?? 30_000);
  }
  if (!batch.results_url) throw new Error(`batch ${created.id} ended without results`);
  const body = await (
    await request("Anthropic API", batch.results_url, { method: "GET", headers }, o)
  ).text();
  const out: (Completion | undefined)[] = Array.from({ length: prompts.length });
  for (const line of body.split("\n")) {
    if (!line.trim()) continue;
    const r = JSON.parse(line) as {
      custom_id: string;
      result: { type: string; message?: AnthropicMessage; error?: unknown };
    };
    // A failed request is an infrastructure error, not a wrong answer, so it must not be scored.
    if (r.result.type !== "succeeded" || !r.result.message)
      throw new Error(
        `batch request ${r.custom_id} ${r.result.type}: ${JSON.stringify(r.result.error)}`,
      );
    // The API is expected to echo ids as `r<n>`; anything else must fail here, not
    // as a mysterious "no result for rNaN" after the loop.
    const at = Number(r.custom_id.slice(1));
    if (!Number.isInteger(at) || at < 0 || at >= out.length)
      throw new Error(
        `batch ${created.id}: non-conforming custom_id ${JSON.stringify(r.custom_id)}`,
      );
    out[at] = fromAnthropic(r.result.message);
  }
  const missing = out.findIndex((c) => c === undefined);
  if (missing >= 0) throw new Error(`batch ${created.id} has no result for r${missing}`);
  return out as Completion[];
}

/**
 * Turns a batch API into a Provider: calls made while the event loop is busy are collected and
 * sent together, so the same task runner drives both modes. Repair prompts, issued once the
 * first batch resolves, form the second batch.
 */
export function batchingProvider(submit: (prompts: string[]) => Promise<Completion[]>): Provider {
  let queue: { prompt: string; resolve: (c: Completion) => void; reject: (e: unknown) => void }[] =
    [];
  const flush = () => {
    const jobs = queue;
    queue = [];
    submit(jobs.map((j) => j.prompt)).then(
      (cs) => jobs.forEach((j, i) => j.resolve(cs[i] as Completion)),
      (e: unknown) => jobs.forEach((j) => j.reject(e)),
    );
  };
  return {
    complete(prompt) {
      return new Promise((resolve, reject) => {
        if (queue.length === 0) setImmediate(flush);
        queue.push({ prompt, resolve, reject });
      });
    },
  };
}

/** Input tokens of `text` as one user message, per the provider's own tokenizer. */
export async function countAnthropicTokens(o: AnthropicOptions, text: string): Promise<number> {
  const json = (await post(
    "Anthropic API",
    `${API}/messages/count_tokens`,
    anthropicHeaders(o.apiKey),
    { model: o.model, messages: [{ role: "user", content: text }] },
    o,
  )) as { input_tokens?: number };
  if (typeof json.input_tokens !== "number")
    throw new Error("Anthropic token counting returned no input_tokens");
  return json.input_tokens;
}

/** Deterministic stand-in for tests: the reply is a pure function of the prompt. */
export function mockProvider(reply: (prompt: string) => string): Provider {
  return {
    async complete(prompt) {
      const text = reply(prompt);
      return { text, outputTokens: text.length };
    },
  };
}
