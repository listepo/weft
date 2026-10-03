import { countProxy } from "./tokens.ts";
import { CORPUS_DIR, readScreen, type EditTask, type QuestionTask, type Task } from "./corpus.ts";
import { FENCE_LANG, extractDocument, parsers } from "./formats.ts";
import { evaluate, missingContent, norm, type Format } from "./neutral.ts";
import { PRIMERS } from "./primers.ts";
import type { Provider } from "./provider.ts";
import { readFileSync } from "node:fs";

export interface TaskResult {
  id: string;
  screen: string;
  format: Format;
  model: string;
  type: Task["type"];
  success: boolean;
  /** Edit output parsed and passed the format's own validity checks. */
  valid: boolean;
  outputTokens: number;
  errors: string[];
  failed: string[];
}

const dataOf = (screen: string): string =>
  readFileSync(`${CORPUS_DIR}${screen}/data.json`, "utf8").trim();

export function editPrompt(task: EditTask, format: Format): string {
  return [
    PRIMERS[format],
    `Data model:\n\`\`\`json\n${dataOf(task.screen)}\n\`\`\``,
    `Current screen:\n\`\`\`${FENCE_LANG[format]}\n${readScreen(task.screen, format).trim()}\n\`\`\``,
    `Change: ${task.instruction}`,
    "Keep everything else exactly as it is. Reply with the complete updated document in a single code block and nothing else.",
  ].join("\n\n");
}

export function readPrompt(task: QuestionTask, format: Format): string {
  return [
    PRIMERS[format],
    `Data model:\n\`\`\`json\n${dataOf(task.screen)}\n\`\`\``,
    `Screen:\n\`\`\`${FENCE_LANG[format]}\n${readScreen(task.screen, format).trim()}\n\`\`\``,
    `Question: ${task.question}`,
    "Reply with one short line in the form `ANSWER: <answer>` and nothing else.",
  ].join("\n\n");
}

export function checkEdit(
  task: EditTask,
  format: Format,
  reply: string,
): Pick<TaskResult, "success" | "valid" | "errors" | "failed"> {
  const parsed = parsers[format](extractDocument(reply));
  const valid = parsed.tree !== undefined && parsed.errors.length === 0;
  if (!parsed.tree)
    return { success: false, valid, errors: parsed.errors, failed: ["unparseable output"] };
  const failed = evaluate(parsed.tree, task.expect, format)
    .filter((r) => !r.ok)
    .map((r) => JSON.stringify(r.assertion));
  const original = parsers[format](readScreen(task.screen, format)).tree;
  if (original)
    for (const name of missingContent(original, parsed.tree, task.remove ?? [], format))
      failed.push(`lost content: ${name}`);
  return { success: valid && failed.length === 0, valid, errors: parsed.errors, failed };
}

export function checkAnswer(task: QuestionTask, reply: string): boolean {
  const m = [...reply.matchAll(/ANSWER:\s*(.+)/gi)].pop();
  if (!m) return false;
  const given = norm((m[1] as string).replace(/[`'".\s]+$|^[`'"\s]+/g, ""));
  return task.answer.some((a) => norm(a) === given);
}

export async function runTask(
  task: Task,
  format: Format,
  provider: Provider,
  model: string,
): Promise<TaskResult> {
  const prompt = task.type === "edit" ? editPrompt(task, format) : readPrompt(task, format);
  const completion = await provider.complete(prompt);
  const outputTokens = completion.outputTokens ?? countProxy(completion.text);
  const base = { id: task.id, screen: task.screen, format, model, type: task.type, outputTokens };
  if (task.type === "edit") return { ...base, ...checkEdit(task, format, completion.text) };
  const ok = checkAnswer(task, completion.text);
  return {
    ...base,
    success: ok,
    valid: true,
    errors: [],
    failed: ok ? [] : [`answer ${JSON.stringify(completion.text.slice(0, 200))}`],
  };
}

export interface Summary {
  model: string;
  format: Format;
  tasks: number;
  successRate: number;
  validRate: number;
  meanOutputTokens: number;
}

export function summarize(results: TaskResult[]): Summary[] {
  const groups = new Map<string, TaskResult[]>();
  for (const r of results)
    groups.set(`${r.model}\u0000${r.format}`, [
      ...(groups.get(`${r.model}\u0000${r.format}`) ?? []),
      r,
    ]);
  return [...groups.values()].map((g) => ({
    model: (g[0] as TaskResult).model,
    format: (g[0] as TaskResult).format,
    tasks: g.length,
    successRate: g.filter((r) => r.success).length / g.length,
    validRate: g.filter((r) => r.valid).length / g.length,
    meanOutputTokens: g.reduce((s, r) => s + r.outputTokens, 0) / g.length,
  }));
}

export function renderSummary(mode: string, summaries: Summary[]): string {
  const lines = [
    `## ${mode}`,
    "",
    "| Model | Format | Tasks | Success | Valid | Mean output tokens |",
    "| --- | --- | ---: | ---: | ---: | ---: |",
  ];
  for (const s of summaries)
    lines.push(
      `| ${s.model} | ${s.format} | ${s.tasks} | ${(s.successRate * 100).toFixed(1)}% | ${(s.validRate * 100).toFixed(1)}% | ${s.meanOutputTokens.toFixed(0)} |`,
    );
  return lines.join("\n") + "\n";
}
