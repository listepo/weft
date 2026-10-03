import { Buffer } from "node:buffer";
import { countTokens } from "gpt-tokenizer/encoding/o200k_base";
import { SCREENS, readScreen } from "./corpus.ts";
import { FORMATS, type Format } from "./neutral.ts";
import { countAnthropicTokens, type AnthropicOptions } from "./provider.ts";

/** o200k_base is OpenAI's tokenizer, a proxy for Claude's, which is not published. */
export const PROXY_TOKENIZER = "gpt-tokenizer 4.0.0 (o200k_base)";
export const countProxy = (text: string): number => countTokens(text);

export interface Cell {
  bytes: number;
  tokens: number;
  /** Tokens after collapsing every whitespace run to one space, applied to all formats alike. */
  compactTokens: number;
  /** Present only when ANTHROPIC_API_KEY was available. */
  anthropicTokens?: number;
}
export type Row = Record<Format, Cell>;
export interface TokenTable {
  rows: Record<string, Row>;
  anthropicModel?: string;
}

export async function measure(anthropic?: AnthropicOptions): Promise<TokenTable> {
  const rows: Record<string, Row> = {};
  for (const screen of SCREENS) {
    const row = {} as Row;
    for (const format of FORMATS) {
      const text = readScreen(screen, format);
      const cell: Cell = {
        bytes: Buffer.byteLength(text),
        tokens: countProxy(text),
        compactTokens: countProxy(text.replace(/\s+/g, " ").trim()),
      };
      if (anthropic) cell.anthropicTokens = await countAnthropicTokens(anthropic, text);
      row[format] = cell;
    }
    rows[screen] = row;
  }
  return { rows, ...(anthropic ? { anthropicModel: anthropic.model } : {}) };
}

type Metric = "bytes" | "tokens" | "compactTokens" | "anthropicTokens";

const value = (c: Cell, m: Metric): number => c[m] ?? 0;

export function totals(table: TokenTable, m: Metric): Record<Format, number> {
  const t = { weft: 0, html: 0, jsx: 0, a2ui: 0 };
  for (const row of Object.values(table.rows)) for (const f of FORMATS) t[f] += value(row[f], m);
  return t;
}

/** Weft divided by the baseline, per screen, so the spread is visible and not only the total. */
export function ratios(table: TokenTable, m: Metric, baseline: Format): Record<string, number> {
  return Object.fromEntries(
    Object.entries(table.rows).map(([s, row]) => [s, value(row.weft, m) / value(row[baseline], m)]),
  );
}

export function spread(r: Record<string, number>): { min: number; median: number; max: number } {
  const v = Object.values(r).sort((a, b) => a - b);
  const mid = Math.floor(v.length / 2);
  return {
    min: v[0] as number,
    median: v.length % 2 ? (v[mid] as number) : ((v[mid - 1] as number) + (v[mid] as number)) / 2,
    max: v[v.length - 1] as number,
  };
}

const pct = (r: number): string => `${(r * 100).toFixed(0)}%`;
const BASELINES: Format[] = ["html", "jsx", "a2ui"];

function metricSection(table: TokenTable, m: Metric, title: string): string[] {
  const lines = [
    `### ${title}`,
    "",
    "| Screen | weft | html | jsx | a2ui | weft/html | weft/jsx | weft/a2ui |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  for (const [screen, row] of Object.entries(table.rows)) {
    const cells = FORMATS.map((f) => value(row[f], m));
    lines.push(
      `| ${screen} | ${cells.join(" | ")} | ${BASELINES.map((b) => pct(value(row.weft, m) / value(row[b], m))).join(" | ")} |`,
    );
  }
  const t = totals(table, m);
  lines.push(
    `| **total** | ${FORMATS.map((f) => `**${t[f]}**`).join(" | ")} | ${BASELINES.map((b) => `**${pct(t.weft / t[b])}**`).join(" | ")} |`,
  );
  lines.push(
    "",
    "Per-screen weft/baseline ratio (lower is better for Weft):",
    "",
    "| Baseline | min | median | max | total |",
    "| --- | ---: | ---: | ---: | ---: |",
  );
  for (const b of BASELINES) {
    const s = spread(ratios(table, m, b));
    lines.push(
      `| ${b} | ${pct(s.min)} | ${pct(s.median)} | ${pct(s.max)} | ${pct(t.weft / t[b])} |`,
    );
  }
  return lines;
}

export function renderTokenTable(table: TokenTable): string {
  const lines = [
    `Tokens use ${PROXY_TOKENIZER}, a proxy for Claude's tokenizer. Bytes are UTF-8 file sizes.`,
    "",
    ...metricSection(table, "tokens", "Tokens (proxy tokenizer)"),
    "",
    ...metricSection(
      table,
      "compactTokens",
      "Tokens with whitespace collapsed (indentation and newlines removed from every format)",
    ),
    "",
    ...metricSection(table, "bytes", "Bytes"),
  ];
  if (table.anthropicModel)
    lines.push(
      "",
      ...metricSection(
        table,
        "anthropicTokens",
        `Tokens (Anthropic count_tokens, ${table.anthropicModel})`,
      ),
    );
  else lines.push("", "Anthropic token counts: not measured (ANTHROPIC_API_KEY was not set).");
  lines.push("");
  for (const [m, label] of [
    ["tokens", "as committed"],
    ["compactTokens", "whitespace collapsed"],
  ] as const) {
    const t = totals(table, m);
    const gain = 1 - t.weft / t.a2ui;
    lines.push(
      `Stop-criterion check (${label}): Weft uses ${(Math.abs(gain) * 100).toFixed(1)}% ${gain >= 0 ? "fewer" : "more"} tokens than A2UI JSON in total; the criterion asks for at least 25% fewer.`,
    );
  }
  return lines.join("\n") + "\n";
}
