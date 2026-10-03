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
  /** One repair prompt was sent because the first edit reply was invalid. */
  repaired: boolean;
  /** Equal to `success` and `valid` when no repair was needed. */
  successAfterRepair: boolean;
  validAfterRepair: boolean;
  /** First reply only, so formats compare on what the model writes unaided. */
  outputTokens: number;
  /** The model's first reply verbatim, so failures can be audited from the raw results. */
  reply: string;
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

/**
 * A caller in production sees only the validator, not the task's hidden expectations, so the
 * repair prompt carries the format's own diagnostics and nothing else.
 */
export function repairPrompt(
  task: EditTask,
  format: Format,
  reply: string,
  errors: string[],
): string {
  const diagnostics = errors.length ? errors : ["the document could not be parsed"];
  return [
    editPrompt(task, format),
    `Your previous reply:\n\`\`\`${FENCE_LANG[format]}\n${extractDocument(reply)}\n\`\`\``,
    `It is not a valid document:\n${diagnostics.map((e) => `- ${e}`).join("\n")}`,
    "Fix these problems and apply the requested change. Reply with the complete corrected document in a single code block and nothing else.",
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
  const base = {
    id: task.id,
    screen: task.screen,
    format,
    model,
    type: task.type,
    outputTokens,
    reply: completion.text,
  };
  if (task.type === "edit") {
    const first = checkEdit(task, format, completion.text);
    if (first.valid)
      return {
        ...base,
        ...first,
        repaired: false,
        successAfterRepair: first.success,
        validAfterRepair: true,
      };
    const retry = await provider.complete(
      repairPrompt(task, format, completion.text, first.errors),
    );
    const second = checkEdit(task, format, retry.text);
    return {
      ...base,
      ...first,
      repaired: true,
      successAfterRepair: second.success,
      validAfterRepair: second.valid,
      failed: [...first.failed, ...second.failed.map((f) => `after repair: ${f}`)],
      errors: [...first.errors, ...second.errors.map((e) => `after repair: ${e}`)],
    };
  }
  const ok = checkAnswer(task, completion.text);
  return {
    ...base,
    success: ok,
    valid: true,
    repaired: false,
    successAfterRepair: ok,
    validAfterRepair: true,
    errors: [],
    failed: ok ? [] : ["wrong answer"],
  };
}

export interface Summary {
  model: string;
  format: Format;
  tasks: number;
  successRate: number;
  successAfterRepairRate: number;
  validRate: number;
  validAfterRepairRate: number;
  meanOutputTokens: number;
}

export function summarize(results: TaskResult[]): Summary[] {
  const groups = new Map<string, TaskResult[]>();
  for (const r of results)
    groups.set(`${r.model}\u0000${r.format}`, [
      ...(groups.get(`${r.model}\u0000${r.format}`) ?? []),
      r,
    ]);
  const rate = (g: TaskResult[], ok: (r: TaskResult) => boolean) => g.filter(ok).length / g.length;
  return [...groups.values()].map((g) => ({
    model: (g[0] as TaskResult).model,
    format: (g[0] as TaskResult).format,
    tasks: g.length,
    successRate: rate(g, (r) => r.success),
    successAfterRepairRate: rate(g, (r) => r.successAfterRepair),
    validRate: rate(g, (r) => r.valid),
    validAfterRepairRate: rate(g, (r) => r.validAfterRepair),
    meanOutputTokens: g.reduce((s, r) => s + r.outputTokens, 0) / g.length,
  }));
}

export function renderSummary(mode: string, summaries: Summary[]): string {
  const lines = [
    `## ${mode}`,
    "",
    "| Model | Format | Tasks | Valid | Valid after repair | Success | Success after repair | Mean output tokens |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  const pct = (x: number) => `${(x * 100).toFixed(1)}%`;
  for (const s of summaries)
    lines.push(
      `| ${s.model} | ${s.format} | ${s.tasks} | ${pct(s.validRate)} | ${pct(s.validAfterRepairRate)} | ${pct(s.successRate)} | ${pct(s.successAfterRepairRate)} | ${s.meanOutputTokens.toFixed(0)} |`,
    );
  return lines.join("\n") + "\n";
}
