import { countProxy } from "./tokens.ts";
import { CORPUS_DIR, readScreen, type EditTask, type QuestionTask, type Task } from "./corpus.ts";
import { FENCE_LANG, extractDocument, parsers } from "./formats.ts";
import { evaluate, missingContent, norm, type Format } from "./neutral.ts";
import { PRIMERS } from "./primers.ts";
import type { Provider } from "./provider.ts";
import { readFileSync } from "node:fs";
import pLimit from "p-limit";

export interface TaskResult {
  id: string;
  screen: string;
  format: Format;
  model: string;
  type: Task["type"];
  /** Which of the repeated runs of this task and format; spreads are taken across samples. */
  sample: number;
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
  /** The reply to the repair prompt, when one was sent. */
  repairReply?: string;
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

/**
 * The question asks which action fires; each format spells an action its own way (`press:x` in
 * HTML, `actions.x()` in JSX), and naming it in that spelling is the right answer, not a miss.
 */
const actionForms = (s: string): string[] => {
  const bare = s
    .replace(/^(press|submit|change|close)\s*:\s*/, "")
    .replace(/^actions\./, "")
    .replace(/\(\s*\)$/, "");
  // Action names are dotted (`nav.reset`); anything else is not an action and is not unwrapped.
  return bare !== s && /^[\w-]+(\.[\w-]+)+$/.test(bare) ? [s, bare] : [s];
};

export function checkAnswer(task: QuestionTask, reply: string): boolean {
  const m = [...reply.matchAll(/ANSWER:\s*(.+)/gi)].pop();
  if (!m) return false;
  const given = norm((m[1] as string).replace(/[`'".\s]+$|^[`'"\s]+/g, ""));
  const forms = actionForms(given);
  return task.answer.some((a) => forms.includes(norm(a)));
}

/** Scores replies already received; shared by live runs and offline re-scoring. */
export function scoreTask(
  task: Task,
  format: Format,
  model: string,
  sample: number,
  reply: string,
  outputTokens: number,
  repairReply?: string,
): TaskResult {
  const base = { id: task.id, screen: task.screen, format, model, type: task.type, sample };
  if (task.type === "question") {
    const ok = checkAnswer(task, reply);
    return {
      ...base,
      success: ok,
      valid: true,
      repaired: false,
      successAfterRepair: ok,
      validAfterRepair: true,
      outputTokens,
      reply,
      errors: [],
      failed: ok ? [] : ["wrong answer"],
    };
  }
  const first = checkEdit(task, format, reply);
  const common = { ...base, ...first, outputTokens, reply };
  if (first.valid)
    return {
      ...common,
      repaired: false,
      successAfterRepair: first.success,
      validAfterRepair: true,
    };
  if (repairReply === undefined)
    return { ...common, repaired: false, successAfterRepair: false, validAfterRepair: false };
  const second = checkEdit(task, format, repairReply);
  return {
    ...common,
    repaired: true,
    repairReply,
    successAfterRepair: second.success,
    validAfterRepair: second.valid,
    failed: [...first.failed, ...second.failed.map((f) => `after repair: ${f}`)],
    errors: [...first.errors, ...second.errors.map((e) => `after repair: ${e}`)],
  };
}

export async function runTask(
  task: Task,
  format: Format,
  provider: Provider,
  model: string,
  sample = 0,
): Promise<TaskResult> {
  const prompt = task.type === "edit" ? editPrompt(task, format) : readPrompt(task, format);
  const completion = await provider.complete(prompt);
  const outputTokens = completion.outputTokens ?? countProxy(completion.text);
  if (task.type === "edit") {
    const first = checkEdit(task, format, completion.text);
    if (!first.valid) {
      const retry = await provider.complete(
        repairPrompt(task, format, completion.text, first.errors),
      );
      return scoreTask(task, format, model, sample, completion.text, outputTokens, retry.text);
    }
  }
  return scoreTask(task, format, model, sample, completion.text, outputTokens);
}

export interface Job {
  task: Task;
  format: Format;
  model: string;
  sample: number;
}

export const planJobs = (
  tasks: Task[],
  formats: Format[],
  models: string[],
  samples: number,
): Job[] =>
  models.flatMap((model) =>
    tasks.flatMap((task) =>
      formats.flatMap((format) =>
        Array.from({ length: samples }, (_, sample) => ({ task, format, model, sample })),
      ),
    ),
  );

/**
 * Runs jobs `concurrency` at a time. The first failure (quota, outage) stops new jobs; the results
 * already paid for are returned with the error so the caller can save them.
 */
export async function runJobs(
  jobs: Job[],
  providerFor: (model: string) => Provider,
  concurrency: number,
  onResult: (r: TaskResult) => void = () => {},
): Promise<{ results: TaskResult[]; error?: unknown }> {
  const limit = pLimit(concurrency);
  const out: (TaskResult | undefined)[] = Array.from({ length: jobs.length });
  let error: unknown;
  await Promise.allSettled(
    jobs.map((j, i) =>
      limit(async () => {
        if (error !== undefined) return;
        try {
          const r = await runTask(j.task, j.format, providerFor(j.model), j.model, j.sample);
          out[i] = r;
          onResult(r);
        } catch (e) {
          // Queued jobs see the error and return at once; clearQueue would leave their promises
          // unsettled forever.
          error ??= e;
        }
      }),
    ),
  );
  const results = out.filter((r): r is TaskResult => r !== undefined);
  return error === undefined ? { results } : { results, error };
}

/** Re-checks saved replies with the current checkers; results saved without replies stay as they are. */
export function rescore(results: TaskResult[], tasks: Task[]): TaskResult[] {
  const byId = new Map(tasks.map((t) => [t.id, t]));
  return results.map((r) => {
    const task = byId.get(r.id);
    if (!task || typeof r.reply !== "string") return r;
    return scoreTask(
      task,
      r.format,
      r.model,
      r.sample ?? 0,
      r.reply,
      r.outputTokens,
      r.repairReply,
    );
  });
}

/** A rate over all samples, with the lowest and highest single-sample rate as its spread. */
export interface Stat {
  mean: number;
  min: number;
  max: number;
}

export interface Summary {
  model: string;
  format: Format;
  tasks: number;
  samples: number;
  success: Stat;
  successAfterRepair: Stat;
  valid: Stat;
  validAfterRepair: Stat;
  meanOutputTokens: number;
}

const groupBy = <T>(xs: T[], key: (x: T) => string): T[][] => {
  const m = new Map<string, T[]>();
  for (const x of xs) m.set(key(x), [...(m.get(key(x)) ?? []), x]);
  return [...m.values()];
};

export function summarize(results: TaskResult[]): Summary[] {
  return groupBy(results, (r) => `${r.model}\u0000${r.format}`).map((g) => {
    // Results saved before sampling existed carry no sample index; they are one sample.
    const bySample = groupBy(g, (r) => String(r.sample ?? 0));
    const stat = (ok: (r: TaskResult) => boolean): Stat => {
      const rates = bySample.map((s) => s.filter(ok).length / s.length);
      return {
        mean: g.filter(ok).length / g.length,
        min: Math.min(...rates),
        max: Math.max(...rates),
      };
    };
    return {
      model: (g[0] as TaskResult).model,
      format: (g[0] as TaskResult).format,
      tasks: new Set(g.map((r) => r.id)).size,
      samples: bySample.length,
      success: stat((r) => r.success),
      successAfterRepair: stat((r) => r.successAfterRepair),
      valid: stat((r) => r.valid),
      validAfterRepair: stat((r) => r.validAfterRepair),
      meanOutputTokens: g.reduce((s, r) => s + r.outputTokens, 0) / g.length,
    };
  });
}

export function renderSummary(mode: string, summaries: Summary[]): string {
  const lines = [
    `## ${mode}`,
    "",
    "Rates are means over all samples; the range is the lowest and highest single-sample rate.",
    "",
    "| Model | Format | Tasks | Samples | Valid | Valid after repair | Success | Success after repair | Mean output tokens |",
    "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  const pct = (x: number) => `${(x * 100).toFixed(1)}%`;
  const cell = (s: Stat) =>
    s.min === s.max ? pct(s.mean) : `${pct(s.mean)} (${pct(s.min)}–${pct(s.max)})`;
  for (const s of summaries)
    lines.push(
      `| ${s.model} | ${s.format} | ${s.tasks} | ${s.samples} | ${cell(s.valid)} | ${cell(s.validAfterRepair)} | ${cell(s.success)} | ${cell(s.successAfterRepair)} | ${s.meanOutputTokens.toFixed(0)} |`,
    );
  return lines.join("\n") + "\n";
}
