import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parse, stringify } from "@weft/core";
import { coreCatalog } from "@weft/catalog";
import { FILE_NAME } from "./formats.ts";
import type { Assertion, Format, Ref } from "./neutral.ts";

export const CORPUS_DIR = fileURLToPath(new URL("../../corpus/", import.meta.url));

export const SCREENS = [
  "login",
  "signup",
  "settings",
  "data-table",
  "tabs",
  "confirm-dialog",
  "wizard-step",
  "search-results",
  "todo-list",
  "profile",
  "menu",
  "error-state",
] as const;

/** The JSON formats read `screen.weft` as canonical JSON, so the corpus keeps one copy of it. */
export function readScreen(screen: string, format: Format): string {
  if (format !== "weft-json" && format !== "weft-json-constrained")
    return readFileSync(`${CORPUS_DIR}${screen}/${FILE_NAME[format]}`, "utf8");
  const { document, diagnostics } = parse(readScreen(screen, "weft"), { catalog: coreCatalog });
  if (!document) throw new Error(`corpus ${screen}: ${JSON.stringify(diagnostics)}`);
  return stringify(document);
}

export interface EditTask {
  id: string;
  screen: string;
  type: "edit";
  instruction: string;
  expect: Assertion[];
  /** Nodes the edit legitimately removes or renames; everything else must survive. */
  remove?: Ref[];
}

export interface QuestionTask {
  id: string;
  screen: string;
  type: "question";
  question: string;
  /** Accepted final answers, compared case-insensitively. */
  answer: string[];
}

export type Task = EditTask | QuestionTask;

export const loadTasks = (): Task[] =>
  JSON.parse(readFileSync(`${CORPUS_DIR}tasks.json`, "utf8")) as Task[];
