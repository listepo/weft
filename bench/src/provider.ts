export interface Completion {
  text: string;
  inputTokens?: number;
  outputTokens?: number;
}

export interface Provider {
  complete(prompt: string): Promise<Completion>;
}

export const DEFAULT_MODELS = ["claude-sonnet-5-5", "claude-opus-5-5"] as const;

const API = "https://api.anthropic.com/v1";
const VERSION = "2023-06-01";

export interface AnthropicOptions {
  apiKey: string;
  model: string;
  maxTokens?: number;
  fetch?: typeof fetch;
  /** Replaced in tests so retries do not wait. */
  sleep?: (ms: number) => Promise<void>;
}

const headers = (apiKey: string) => ({
  "content-type": "application/json",
  "x-api-key": apiKey,
  "anthropic-version": VERSION,
});

const RETRYABLE = new Set([429, 500, 502, 503, 529]);

async function post(
  url: string,
  apiKey: string,
  body: unknown,
  o: Pick<AnthropicOptions, "fetch" | "sleep">,
): Promise<unknown> {
  const doFetch = o.fetch ?? fetch;
  const sleep = o.sleep ?? ((ms: number) => new Promise<void>((r) => setTimeout(r, ms)));
  for (let attempt = 0; ; attempt++) {
    const res = await doFetch(url, {
      method: "POST",
      headers: headers(apiKey),
      body: JSON.stringify(body),
    });
    if (res.ok) return res.json();
    if (attempt < 3 && RETRYABLE.has(res.status)) {
      await sleep(1000 * 2 ** attempt);
      continue;
    }
    throw new Error(`Anthropic API ${res.status}: ${(await res.text()).slice(0, 500)}`);
  }
}

export function anthropicProvider(o: AnthropicOptions): Provider {
  return {
    async complete(prompt) {
      const json = (await post(
        `${API}/messages`,
        o.apiKey,
        {
          model: o.model,
          max_tokens: o.maxTokens ?? 8192,
          messages: [{ role: "user", content: prompt }],
        },
        o,
      )) as {
        content?: { type: string; text?: string }[];
        usage?: { input_tokens?: number; output_tokens?: number };
      };
      const text = (json.content ?? [])
        .filter((b) => b.type === "text")
        .map((b) => b.text ?? "")
        .join("");
      return {
        text,
        ...(json.usage?.input_tokens !== undefined ? { inputTokens: json.usage.input_tokens } : {}),
        ...(json.usage?.output_tokens !== undefined
          ? { outputTokens: json.usage.output_tokens }
          : {}),
      };
    },
  };
}

/** Input tokens of `text` as one user message, per the provider's own tokenizer. */
export async function countAnthropicTokens(o: AnthropicOptions, text: string): Promise<number> {
  const json = (await post(
    `${API}/messages/count_tokens`,
    o.apiKey,
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
